use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::CURRENT_TIME;

use crate::icon::{BadIcon, Icon};
use crate::tray::{
    MouseButton, MouseButtonState, Rect, TrayIconAttributes, TrayIconEvent, TrayIconId,
};

x11rb::atom_manager! {
    pub TrayAtoms: TrayAtomsCookie {
        _NET_SYSTEM_TRAY_VISUAL,
        _NET_SYSTEM_TRAY_S0,
        _NET_SYSTEM_TRAY_OPCODE,
        _XEMBED,
        _XEMBED_INFO,
        MANAGER,
        VISUALID,
    }
}

const SYSTEM_TRAY_REQUEST_DOCK: u32 = 0;
const XEMBED_MAPPED: u32 = 1;
const ICON_SIZE: u16 = 24;

#[derive(Debug, Clone)]
pub struct PlatformIcon {
    pub(crate) rgba: Vec<u8>,
    pub(crate) width: i32,
    pub(crate) height: i32,
}

const PIXEL_SIZE: usize = 4;

impl PlatformIcon {
    pub fn from_rgba(rgba: Vec<u8>, width: u32, height: u32) -> Result<Self, BadIcon> {
        if !rgba.len().is_multiple_of(PIXEL_SIZE) {
            return Err(BadIcon::ByteCountNotDivisibleBy4 {
                byte_count: rgba.len(),
            });
        }
        let pixel_count = rgba.len() / PIXEL_SIZE;
        if pixel_count != (width * height) as usize {
            return Err(BadIcon::DimensionsVsPixelCount {
                width,
                height,
                width_x_height: (width * height) as usize,
                pixel_count,
            });
        }
        Ok(Self {
            rgba,
            width: width as i32,
            height: height as i32,
        })
    }
}

struct IconData {
    rgba: Vec<u8>,
    width: u32,
    height: u32,
}

pub struct TrayIconImpl {
    conn: Arc<RustConnection>,
    screen_num: usize,
    window: Window,
    pixmap: Pixmap,
    window_gc: Gcontext,
    pixmap_gc: Gcontext,
    colormap: Colormap,
    depth: u8,
    temp_dir_path: Option<PathBuf>,
    icon_data: Arc<Mutex<Option<IconData>>>,
    running: Arc<AtomicBool>,
    event_thread: Option<thread::JoinHandle<()>>,
}

impl TrayIconImpl {
    pub fn new(id: TrayIconId, attrs: TrayIconAttributes) -> crate::Result<Self> {
        let (conn, screen_num) = RustConnection::connect(None)?;
        let conn = Arc::new(conn);
        let screen = &conn.setup().roots[screen_num];
        let atoms = TrayAtoms::new(&conn)?.reply()?;

        let tray_owner = conn.get_selection_owner(atoms._NET_SYSTEM_TRAY_S0)?.reply()?.owner;

        if tray_owner == x11rb::NONE {
            return Err(crate::Error::NotSupported("The current environment does not have a system tray"));
        }

        let mut visual = screen.root_visual;
        let mut depth = screen.root_depth;

        'outer: for depth_type in &screen.allowed_depths {
            if depth_type.depth == 32 {
                for visual_type in &depth_type.visuals {
                    if visual_type.class == VisualClass::TRUE_COLOR {
                        visual = visual_type.visual_id;
                        depth = depth_type.depth;

                        break 'outer;
                    }
                }
            }
        }

        {
            let reply = conn.get_property(
                false,
                tray_owner,
                atoms._NET_SYSTEM_TRAY_VISUAL,
                atoms.VISUALID,
                0,
                size_of::<Visualid>() as u32
            )?.reply()?;

            let mut tray_visual = reply.value32();

            match tray_visual.as_mut().and_then(Iterator::next) {
                Some(tray_visual) => visual = tray_visual,
                None => {},
            }
        }

        let pixmap = conn.generate_id()?;
        conn.create_pixmap(depth, pixmap, screen.root, ICON_SIZE, ICON_SIZE)?;

        let colormap = conn.generate_id()?;
        conn.create_colormap(ColormapAlloc::NONE, colormap, screen.root, visual)?;

        let window = conn.generate_id()?;
        conn.create_window(
            depth,
            window,
            screen.root,
            0,
            0,
            ICON_SIZE,
            ICON_SIZE,
            0,
            WindowClass::INPUT_OUTPUT,
            visual,
            &CreateWindowAux::new()
                .background_pixel(0)
                .border_pixel(0)
                .colormap(colormap)
                .event_mask(
                    EventMask::EXPOSURE
                        | EventMask::BUTTON_PRESS
                        | EventMask::BUTTON_RELEASE
                        | EventMask::ENTER_WINDOW
                        | EventMask::LEAVE_WINDOW
                        | EventMask::POINTER_MOTION
                        | EventMask::STRUCTURE_NOTIFY,
                ),
        )?;

        let window_gc = conn.generate_id()?;
        conn.create_gc(window_gc, window, &CreateGCAux::new())?;

        let pixmap_gc = conn.generate_id()?;
        conn.create_gc(pixmap_gc, pixmap, &CreateGCAux::new())?;

        conn.change_property32(
            PropMode::REPLACE,
            window,
            atoms._XEMBED_INFO,
            atoms._XEMBED_INFO,
            &[0, XEMBED_MAPPED],
        )?;

        Self::send_dock_request(&conn, tray_owner, window, &atoms)?;

        conn.flush()?;

        let icon_data = Arc::new(Mutex::new(None));

        if let Some(ref icon) = attrs.icon {
            let mut data = icon_data.lock().unwrap();
            *data = Some(IconData {
                rgba: icon.inner.rgba.clone(),
                width: icon.inner.width as u32,
                height: icon.inner.height as u32,
            });
        }

        let running = Arc::new(AtomicBool::new(true));
        let event_thread = {
            let conn = Arc::clone(&conn);
            let running = Arc::clone(&running);
            let id = id.clone();
            let icon_data = Arc::clone(&icon_data);
            thread::spawn(move || {
                Self::event_loop(conn, screen_num, window, window_gc, pixmap, pixmap_gc, depth, id, running, icon_data);
            })
        };

        Ok(Self {
            conn,
            screen_num,
            window,
            window_gc,
            pixmap,
            pixmap_gc,
            colormap,
            depth,
            temp_dir_path: attrs.temp_dir_path,
            icon_data,
            running,
            event_thread: Some(event_thread),
        })
    }

    fn send_dock_request(
        conn: &RustConnection,
        tray_owner: Window,
        icon_window: Window,
        atoms: &TrayAtoms,
    ) -> crate::Result<()> {
        let event = ClientMessageEvent::new(
            32,
            tray_owner,
            atoms._NET_SYSTEM_TRAY_OPCODE,
            [CURRENT_TIME, SYSTEM_TRAY_REQUEST_DOCK, icon_window, 0, 0],
        );
        conn.send_event(false, tray_owner, EventMask::NO_EVENT, event)?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn event_loop(
        conn: Arc<RustConnection>,
        screen_num: usize,
        window: Window,
        window_gc: Gcontext,
        pixmap: Pixmap,
        pixmap_gc: Gcontext,
        depth: u8,
        id: TrayIconId,
        running: Arc<AtomicBool>,
        icon_data: Arc<Mutex<Option<IconData>>>,
    ) {
        while running.load(Ordering::Relaxed) {
            match conn.poll_for_event() {
                Ok(Some(event)) => {
                    match &event {
                        x11rb::protocol::Event::Expose(e) if e.window == window => {
                            Self::draw_icon(&conn, screen_num, window, window_gc, pixmap, pixmap_gc, depth, &icon_data);
                        }
                        x11rb::protocol::Event::ConfigureNotify(e) if e.window == window => {
                            Self::draw_icon(&conn, screen_num, window, window_gc, pixmap, pixmap_gc, depth, &icon_data);
                        }
                        _ => {}
                    }
                    if let Some(tray_event) = Self::handle_event(&event, window, &id) {
                        TrayIconEvent::send(tray_event);
                    }
                }
                Ok(None) => {
                    thread::sleep(std::time::Duration::from_millis(16));
                }
                Err(_) => break,
            }
        }
    }

    fn draw_icon(
        conn: &RustConnection,
        screen_num: usize,
        window: Window,
        window_gc: Gcontext,
        pixmap: Pixmap,
        pixmap_gc: Gcontext,
        depth: u8,
        icon_data: &Arc<Mutex<Option<IconData>>>,
    ) {
        let data = icon_data.lock().unwrap();
        if let Some(ref icon) = *data {
            let geom = match conn.get_geometry(window) {
                Ok(cookie) => match cookie.reply() {
                    Ok(g) => g,
                    Err(_) => return,
                },
                Err(_) => return,
            };

            let win_width = geom.width as u32;
            let win_height = geom.height as u32;

            let scaled = scale_icon(&icon.rgba, icon.width, icon.height, win_width, win_height);
            let bgrx = rgba_to_bgrx(&scaled);

            let _ = conn.put_image(
                ImageFormat::Z_PIXMAP,
                pixmap,
                pixmap_gc,
                win_width as u16,
                win_height as u16,
                0,
                0,
                0,
                depth,
                &bgrx,
            );

            let _ = conn.change_gc(window_gc, &ChangeGCAux::new()
                .clip_mask(pixmap)
                .clip_x_origin(0)
                .clip_y_origin(0));

            let _ = conn.put_image(
                ImageFormat::Z_PIXMAP,
                window,
                window_gc,
                win_width as u16,
                win_height as u16,
                0,
                0,
                0,
                depth,
                &bgrx,
            );

            let _ = conn.flush();
        } else {
            let screen = &conn.setup().roots[screen_num];
            let geom = match conn.get_geometry(window) {
                Ok(cookie) => match cookie.reply() {
                    Ok(g) => g,
                    Err(_) => return,
                },
                Err(_) => return,
            };
            let _ = conn.change_gc(window_gc, &ChangeGCAux::new().foreground(screen.black_pixel));
            let rect = Rectangle {
                x: 0,
                y: 0,
                width: geom.width,
                height: geom.height,
            };
            let _ = conn.poly_fill_rectangle(window, window_gc, &[rect]);
            let _ = conn.flush();
        }
    }

    fn handle_event(event: &x11rb::protocol::Event, window: Window, id: &TrayIconId) -> Option<TrayIconEvent> {
        match event {
            x11rb::protocol::Event::ButtonPress(e) if e.event == window => {
                let button = match e.detail {
                    1 => MouseButton::Left,
                    2 => MouseButton::Middle,
                    3 => MouseButton::Right,
                    _ => MouseButton::Left,
                };
                Some(TrayIconEvent::Click {
                    id: id.clone(),
                    position: dpi::PhysicalPosition::new(e.root_x as f64, e.root_y as f64),
                    rect: Rect::default(),
                    button,
                    button_state: MouseButtonState::Down,
                })
            }
            x11rb::protocol::Event::ButtonRelease(e) if e.event == window => {
                let button = match e.detail {
                    1 => MouseButton::Left,
                    2 => MouseButton::Middle,
                    3 => MouseButton::Right,
                    _ => MouseButton::Left,
                };
                Some(TrayIconEvent::Click {
                    id: id.clone(),
                    position: dpi::PhysicalPosition::new(e.root_x as f64, e.root_y as f64),
                    rect: Rect::default(),
                    button,
                    button_state: MouseButtonState::Up,
                })
            }
            x11rb::protocol::Event::EnterNotify(e) if e.event == window => {
                Some(TrayIconEvent::Enter {
                    id: id.clone(),
                    position: dpi::PhysicalPosition::new(e.root_x as f64, e.root_y as f64),
                    rect: Rect::default(),
                })
            }
            x11rb::protocol::Event::LeaveNotify(e) if e.event == window => {
                Some(TrayIconEvent::Leave {
                    id: id.clone(),
                    position: dpi::PhysicalPosition::new(e.root_x as f64, e.root_y as f64),
                    rect: Rect::default(),
                })
            }
            x11rb::protocol::Event::MotionNotify(e) if e.event == window => {
                Some(TrayIconEvent::Move {
                    id: id.clone(),
                    position: dpi::PhysicalPosition::new(e.root_x as f64, e.root_y as f64),
                    rect: Rect::default(),
                })
            }
            _ => None,
        }
    }

    pub fn set_icon(&mut self, icon: Option<Icon>) -> crate::Result<()> {
        {
            let mut data = self.icon_data.lock().unwrap();
            *data = icon.map(|i| IconData {
                rgba: i.inner.rgba.clone(),
                width: i.inner.width as u32,
                height: i.inner.height as u32,
            });
        }

        let screen = &self.conn.setup().roots[self.screen_num];
        Self::draw_icon(&self.conn, self.screen_num, self.window, self.window_gc, self.pixmap, self.pixmap_gc, self.depth, &self.icon_data);

        Ok(())
    }

    pub fn set_tooltip<S: AsRef<str>>(&mut self, _tooltip: Option<S>) -> crate::Result<()> {
        Ok(())
    }

    pub fn set_title<S: AsRef<str>>(&mut self, _title: Option<S>) {
    }

    pub fn set_visible(&mut self, visible: bool) -> crate::Result<()> {
        if visible {
            self.conn.map_window(self.window)?;
        } else {
            self.conn.unmap_window(self.window)?;
        }
        self.conn.flush()?;
        Ok(())
    }

    pub fn set_temp_dir_path<P: AsRef<Path>>(&mut self, path: Option<P>) {
        self.temp_dir_path = path.map(|p| p.as_ref().to_path_buf());
    }

    pub fn rect(&self) -> Option<Rect> {
        if let Ok(geom) = self.conn.get_geometry(self.window)
            && let Ok(reply) = geom.reply()
        {
            return Some(Rect {
                size: dpi::PhysicalSize::new(reply.width as u32, reply.height as u32),
                position: dpi::PhysicalPosition::new(reply.x as f64, reply.y as f64),
            });
        }
        None
    }
}

impl Drop for TrayIconImpl {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.event_thread.take() {
            let _ = handle.join();
        }
        let _ = self.conn.destroy_window(self.window);
        let _ = self.conn.free_gc(self.window_gc);
        let _ = self.conn.free_pixmap(self.pixmap);
        let _ = self.conn.free_gc(self.pixmap_gc);
        let _ = self.conn.free_colormap(self.colormap);
        let _ = self.conn.flush();
    }
}

fn rgba_to_bgrx(rgba: &[u8]) -> Vec<u8> {
    let mut bgrx = Vec::with_capacity(rgba.len());
    for chunk in rgba.chunks(4) {
        let a = chunk[3] as u32;
        let r = (chunk[0] as u32 * a / 255) as u8;
        let g = (chunk[1] as u32 * a / 255) as u8;
        let b = (chunk[2] as u32 * a / 255) as u8;
        bgrx.push(b);
        bgrx.push(g);
        bgrx.push(r);
        bgrx.push(0);
    }
    bgrx
}

fn scale_icon(rgba: &[u8], src_w: u32, src_h: u32, dst_w: u32, dst_h: u32) -> Vec<u8> {
    if src_w == dst_w && src_h == dst_h {
        return rgba.to_vec();
    }

    let mut result = vec![0u8; (dst_w * dst_h * 4) as usize];

    for y in 0..dst_h {
        for x in 0..dst_w {
            let src_x = (x * src_w / dst_w).min(src_w - 1);
            let src_y = (y * src_h / dst_h).min(src_h - 1);

            let src_idx = ((src_y * src_w + src_x) * 4) as usize;
            let dst_idx = ((y * dst_w + x) * 4) as usize;

            result[dst_idx] = rgba[src_idx];
            result[dst_idx + 1] = rgba[src_idx + 1];
            result[dst_idx + 2] = rgba[src_idx + 2];
            result[dst_idx + 3] = rgba[src_idx + 3];
        }
    }

    result
}
