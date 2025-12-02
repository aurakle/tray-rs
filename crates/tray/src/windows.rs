use std::{fmt, io, mem, path::Path, ptr, sync::Arc};

use once_cell::sync::Lazy;
use windows_sys::{
    core::PCWSTR,
    s,
    Win32::{
        Foundation::{FALSE, HWND, LPARAM, LRESULT, POINT, RECT, S_OK, TRUE, WPARAM},
        UI::{
            Shell::{
                Shell_NotifyIconGetRect, Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP,
                NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW, NOTIFYICONIDENTIFIER,
            },
            WindowsAndMessaging::{
                CreateIcon, CreateWindowExW, DefWindowProcW, DestroyIcon, DestroyWindow,
                GetCursorPos, KillTimer, LoadImageW, RegisterClassW, RegisterWindowMessageA,
                SendMessageW, SetTimer, CREATESTRUCTW, CW_USEDEFAULT, GWL_USERDATA, HICON,
                IMAGE_ICON, LR_DEFAULTSIZE, LR_LOADFROMFILE, WINDOW_LONG_PTR_INDEX, WM_CREATE,
                WM_DESTROY, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDBLCLK,
                WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEMOVE, WM_NCCREATE, WM_RBUTTONDBLCLK,
                WM_RBUTTONDOWN, WM_RBUTTONUP, WM_TIMER, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE,
                WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT, WS_OVERLAPPED,
            },
        },
    },
};

use std::sync::atomic::{AtomicU32, Ordering};

use crate::{
    icon::{BadIcon, Icon},
    tray::{MouseButton, MouseButtonState, Rect, TrayIconAttributes, TrayIconEvent, TrayIconId},
};

static COUNTER: AtomicU32 = AtomicU32::new(1);

const WM_USER_TRAYICON: u32 = 6002;
const WM_USER_UPDATE_TRAYICON: u32 = 6004;
const WM_USER_SHOW_TRAYICON: u32 = 6005;
const WM_USER_HIDE_TRAYICON: u32 = 6006;
const WM_USER_UPDATE_TRAYTOOLTIP: u32 = 6007;
const WM_USER_LEAVE_TIMER_ID: u32 = 6008;

static S_U_TASKBAR_RESTART: Lazy<u32> =
    Lazy::new(|| unsafe { RegisterWindowMessageA(s!("TaskbarCreated")) });

const PIXEL_SIZE: usize = 4;

#[repr(C)]
struct Pixel {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

impl Pixel {
    fn convert_to_bgra(&mut self) {
        mem::swap(&mut self.r, &mut self.b);
    }
}

struct RgbaIcon {
    rgba: Vec<u8>,
    width: u32,
    height: u32,
}

impl RgbaIcon {
    fn from_rgba(rgba: Vec<u8>, width: u32, height: u32) -> Result<Self, BadIcon> {
        if rgba.len() % PIXEL_SIZE != 0 {
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
        Ok(Self { rgba, width, height })
    }

    fn into_windows_icon(self) -> Result<PlatformIcon, BadIcon> {
        let rgba = self.rgba;
        let pixel_count = rgba.len() / PIXEL_SIZE;
        let mut and_mask = Vec::with_capacity(pixel_count);
        let pixels =
            unsafe { std::slice::from_raw_parts_mut(rgba.as_ptr() as *mut Pixel, pixel_count) };
        for pixel in pixels {
            and_mask.push(pixel.a.wrapping_sub(u8::MAX));
            pixel.convert_to_bgra();
        }
        assert_eq!(and_mask.len(), pixel_count);
        let handle = unsafe {
            CreateIcon(
                ptr::null_mut(),
                self.width as i32,
                self.height as i32,
                1,
                (PIXEL_SIZE * 8) as u8,
                and_mask.as_ptr(),
                rgba.as_ptr(),
            )
        };
        if !handle.is_null() {
            Ok(PlatformIcon::from_handle(handle))
        } else {
            Err(BadIcon::OsError(io::Error::last_os_error()))
        }
    }
}

#[derive(Debug)]
struct RaiiIcon {
    handle: HICON,
}

#[derive(Clone)]
pub struct PlatformIcon {
    inner: Arc<RaiiIcon>,
}

unsafe impl Send for PlatformIcon {}

impl PlatformIcon {
    pub fn as_raw_handle(&self) -> HICON {
        self.inner.handle
    }

    pub fn from_rgba(rgba: Vec<u8>, width: u32, height: u32) -> Result<Self, BadIcon> {
        let rgba_icon = RgbaIcon::from_rgba(rgba, width, height)?;
        rgba_icon.into_windows_icon()
    }

    pub fn from_handle(handle: HICON) -> Self {
        Self {
            #[allow(clippy::arc_with_non_send_sync)]
            inner: Arc::new(RaiiIcon { handle }),
        }
    }

    pub fn from_path<P: AsRef<Path>>(path: P, size: Option<(u32, u32)>) -> Result<Self, BadIcon> {
        let (width, height) = size.unwrap_or((0, 0));
        let wide_path = encode_wide(path.as_ref());

        let handle = unsafe {
            LoadImageW(
                ptr::null_mut(),
                wide_path.as_ptr(),
                IMAGE_ICON,
                width as i32,
                height as i32,
                LR_DEFAULTSIZE | LR_LOADFROMFILE,
            )
        };
        if !handle.is_null() {
            Ok(PlatformIcon::from_handle(handle as HICON))
        } else {
            Err(BadIcon::OsError(io::Error::last_os_error()))
        }
    }

    fn from_resource_inner_name(name: PCWSTR, size: Option<(u32, u32)>) -> Result<Self, BadIcon> {
        let (width, height) = size.unwrap_or((0, 0));
        let handle = unsafe {
            LoadImageW(
                get_instance_handle(),
                name,
                IMAGE_ICON,
                width as i32,
                height as i32,
                LR_DEFAULTSIZE,
            )
        };
        if !handle.is_null() {
            Ok(PlatformIcon::from_handle(handle as HICON))
        } else {
            Err(BadIcon::OsError(io::Error::last_os_error()))
        }
    }

    pub fn from_resource(resource_id: u16, size: Option<(u32, u32)>) -> Result<Self, BadIcon> {
        Self::from_resource_inner_name(resource_id as PCWSTR, size)
    }

    pub fn from_resource_name(
        resource_name: &str,
        size: Option<(u32, u32)>,
    ) -> Result<Self, BadIcon> {
        let wide_name = encode_wide(resource_name);
        Self::from_resource_inner_name(wide_name.as_ptr(), size)
    }
}

impl Drop for RaiiIcon {
    fn drop(&mut self) {
        unsafe { DestroyIcon(self.handle) };
    }
}

impl fmt::Debug for PlatformIcon {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        (*self.inner).fmt(formatter)
    }
}

fn encode_wide<S: AsRef<std::ffi::OsStr>>(string: S) -> Vec<u16> {
    std::os::windows::prelude::OsStrExt::encode_wide(string.as_ref())
        .chain(std::iter::once(0))
        .collect()
}

fn get_instance_handle() -> windows_sys::Win32::Foundation::HMODULE {
    unsafe extern "C" {
        static __ImageBase: windows_sys::Win32::System::SystemServices::IMAGE_DOS_HEADER;
    }
    unsafe { &__ImageBase as *const _ as _ }
}

#[inline(always)]
unsafe fn get_window_long(hwnd: HWND, nindex: WINDOW_LONG_PTR_INDEX) -> isize {
    #[cfg(target_pointer_width = "64")]
    return unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(hwnd, nindex) };
    #[cfg(target_pointer_width = "32")]
    return unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::GetWindowLongW(hwnd, nindex) as isize
    };
}

#[inline(always)]
unsafe fn set_window_long(hwnd: HWND, nindex: WINDOW_LONG_PTR_INDEX, dwnewlong: isize) -> isize {
    #[cfg(target_pointer_width = "64")]
    return unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::SetWindowLongPtrW(hwnd, nindex, dwnewlong)
    };
    #[cfg(target_pointer_width = "32")]
    return unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::SetWindowLongW(hwnd, nindex, dwnewlong as i32)
            as isize
    };
}

struct TrayUserData {
    internal_id: u32,
    id: TrayIconId,
    hwnd: HWND,
    icon: Option<Icon>,
    tooltip: Option<String>,
    entered: bool,
    last_position: Option<dpi::PhysicalPosition<f64>>,
}

pub struct TrayIconImpl {
    hwnd: HWND,
    internal_id: u32,
}

impl TrayIconImpl {
    pub fn new(id: TrayIconId, attrs: TrayIconAttributes) -> crate::Result<Self> {
        let internal_id = COUNTER.fetch_add(1, Ordering::Relaxed);

        let class_name = encode_wide("tray_icon_app");
        unsafe {
            let hinstance = get_instance_handle();

            let wnd_class = WNDCLASSW {
                lpfnWndProc: Some(tray_proc),
                lpszClassName: class_name.as_ptr(),
                hInstance: hinstance,
                ..std::mem::zeroed()
            };

            RegisterClassW(&wnd_class);

            let traydata = TrayUserData {
                id,
                internal_id,
                hwnd: ptr::null_mut(),
                icon: attrs.icon.clone(),
                tooltip: attrs.tooltip.clone(),
                entered: false,
                last_position: None,
            };

            let hwnd = CreateWindowExW(
                WS_EX_NOACTIVATE | WS_EX_TRANSPARENT | WS_EX_LAYERED | WS_EX_TOOLWINDOW,
                class_name.as_ptr(),
                ptr::null(),
                WS_OVERLAPPED,
                CW_USEDEFAULT,
                0,
                CW_USEDEFAULT,
                0,
                ptr::null_mut(),
                ptr::null_mut(),
                hinstance,
                Box::into_raw(Box::new(traydata)) as _,
            );
            if hwnd.is_null() {
                return Err(crate::Error::OsError(io::Error::last_os_error()));
            }

            let hicon = attrs.icon.as_ref().map(|i| i.inner.as_raw_handle());

            if !register_tray_icon(hwnd, internal_id, &hicon, &attrs.tooltip) {
                return Err(crate::Error::OsError(io::Error::last_os_error()));
            }

            Ok(Self { hwnd, internal_id })
        }
    }

    pub fn set_icon(&mut self, icon: Option<Icon>) -> crate::Result<()> {
        unsafe {
            let mut nid = NOTIFYICONDATAW {
                uFlags: NIF_ICON,
                hWnd: self.hwnd,
                uID: self.internal_id,
                ..std::mem::zeroed()
            };

            if let Some(hicon) = icon.as_ref().map(|i| i.inner.as_raw_handle()) {
                nid.hIcon = hicon;
            }

            if Shell_NotifyIconW(NIM_MODIFY, &mut nid as _) == 0 {
                return Err(crate::Error::OsError(io::Error::last_os_error()));
            }

            SendMessageW(
                self.hwnd,
                WM_USER_UPDATE_TRAYICON,
                Box::into_raw(Box::new(icon)) as _,
                0,
            );
        }

        Ok(())
    }

    pub fn set_tooltip<S: AsRef<str>>(&mut self, tooltip: Option<S>) -> crate::Result<()> {
        unsafe {
            let mut nid = NOTIFYICONDATAW {
                uFlags: NIF_TIP,
                hWnd: self.hwnd,
                uID: self.internal_id,
                ..std::mem::zeroed()
            };
            if let Some(tooltip) = &tooltip {
                let tip = encode_wide(tooltip.as_ref());
                #[allow(clippy::manual_memcpy)]
                for i in 0..tip.len().min(128) {
                    nid.szTip[i] = tip[i];
                }
            }

            if Shell_NotifyIconW(NIM_MODIFY, &mut nid as _) == 0 {
                return Err(crate::Error::OsError(io::Error::last_os_error()));
            }

            SendMessageW(
                self.hwnd,
                WM_USER_UPDATE_TRAYTOOLTIP,
                Box::into_raw(Box::new(tooltip.map(|t| t.as_ref().to_string()))) as _,
                0,
            );
        }

        Ok(())
    }

    pub fn set_title<S: AsRef<str>>(&mut self, _title: Option<S>) {}

    pub fn set_visible(&mut self, visible: bool) -> crate::Result<()> {
        unsafe {
            SendMessageW(
                self.hwnd,
                if visible {
                    WM_USER_SHOW_TRAYICON
                } else {
                    WM_USER_HIDE_TRAYICON
                },
                0,
                0,
            );
        }

        Ok(())
    }

    pub fn rect(&self) -> Option<Rect> {
        get_tray_rect(self.internal_id, self.hwnd).map(Into::into)
    }
}

impl Drop for TrayIconImpl {
    fn drop(&mut self) {
        unsafe {
            remove_tray_icon(self.hwnd, self.internal_id);
            DestroyWindow(self.hwnd);
        }
    }
}

unsafe extern "system" fn tray_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let userdata_ptr = unsafe { get_window_long(hwnd, GWL_USERDATA) };
    let userdata_ptr = match (userdata_ptr, msg) {
        (0, WM_NCCREATE) => {
            let createstruct = unsafe { &mut *(lparam as *mut CREATESTRUCTW) };
            let userdata = unsafe { &mut *(createstruct.lpCreateParams as *mut TrayUserData) };
            userdata.hwnd = hwnd;
            unsafe { set_window_long(hwnd, GWL_USERDATA, createstruct.lpCreateParams as _) };
            return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
        }
        (0, WM_CREATE) => return -1,
        (_, WM_CREATE) => return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        (0, _) => return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        _ => userdata_ptr as *mut TrayUserData,
    };

    let userdata = unsafe { &mut *(userdata_ptr) };

    match msg {
        WM_DESTROY => {
            drop(unsafe { Box::from_raw(userdata_ptr) });
            return 0;
        }
        WM_USER_UPDATE_TRAYICON => {
            let icon = unsafe { Box::from_raw(wparam as *mut Option<Icon>) };
            userdata.icon = *icon;
        }
        WM_USER_SHOW_TRAYICON => {
            unsafe {
                register_tray_icon(
                    userdata.hwnd,
                    userdata.internal_id,
                    &userdata.icon.as_ref().map(|i| i.inner.as_raw_handle()),
                    &userdata.tooltip,
                )
            };
        }
        WM_USER_HIDE_TRAYICON => {
            unsafe { remove_tray_icon(userdata.hwnd, userdata.internal_id) };
        }
        WM_USER_UPDATE_TRAYTOOLTIP => {
            let tooltip = unsafe { Box::from_raw(wparam as *mut Option<String>) };
            userdata.tooltip = *tooltip;
        }
        _ if msg == *S_U_TASKBAR_RESTART => {
            unsafe {
                remove_tray_icon(userdata.hwnd, userdata.internal_id);
                register_tray_icon(
                    userdata.hwnd,
                    userdata.internal_id,
                    &userdata.icon.as_ref().map(|i| i.inner.as_raw_handle()),
                    &userdata.tooltip,
                )
            };
        }

        WM_USER_TRAYICON
            if matches!(
                lparam as u32,
                WM_LBUTTONDOWN
                    | WM_RBUTTONDOWN
                    | WM_MBUTTONDOWN
                    | WM_LBUTTONUP
                    | WM_RBUTTONUP
                    | WM_MBUTTONUP
                    | WM_LBUTTONDBLCLK
                    | WM_RBUTTONDBLCLK
                    | WM_MBUTTONDBLCLK
                    | WM_MOUSEMOVE
            ) =>
        {
            let mut cursor = POINT { x: 0, y: 0 };
            if unsafe { GetCursorPos(&mut cursor as _) } == 0 {
                return 0;
            }

            let id = userdata.id.clone();
            let position = dpi::PhysicalPosition::new(cursor.x as f64, cursor.y as f64);

            let rect = match get_tray_rect(userdata.internal_id, hwnd) {
                Some(rect) => Rect::from(rect),
                None => return 0,
            };

            let event = match lparam as u32 {
                WM_LBUTTONDOWN => TrayIconEvent::Click {
                    id,
                    rect,
                    position,
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Down,
                },
                WM_RBUTTONDOWN => TrayIconEvent::Click {
                    id,
                    rect,
                    position,
                    button: MouseButton::Right,
                    button_state: MouseButtonState::Down,
                },
                WM_MBUTTONDOWN => TrayIconEvent::Click {
                    id,
                    rect,
                    position,
                    button: MouseButton::Middle,
                    button_state: MouseButtonState::Down,
                },
                WM_LBUTTONUP => TrayIconEvent::Click {
                    id,
                    rect,
                    position,
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                },
                WM_RBUTTONUP => TrayIconEvent::Click {
                    id,
                    rect,
                    position,
                    button: MouseButton::Right,
                    button_state: MouseButtonState::Up,
                },
                WM_MBUTTONUP => TrayIconEvent::Click {
                    id,
                    rect,
                    position,
                    button: MouseButton::Middle,
                    button_state: MouseButtonState::Up,
                },
                WM_LBUTTONDBLCLK => TrayIconEvent::DoubleClick {
                    id,
                    rect,
                    position,
                    button: MouseButton::Left,
                },
                WM_RBUTTONDBLCLK => TrayIconEvent::DoubleClick {
                    id,
                    rect,
                    position,
                    button: MouseButton::Right,
                },
                WM_MBUTTONDBLCLK => TrayIconEvent::DoubleClick {
                    id,
                    rect,
                    position,
                    button: MouseButton::Middle,
                },
                WM_MOUSEMOVE if !userdata.entered => {
                    userdata.entered = true;
                    TrayIconEvent::Enter { id, rect, position }
                }
                WM_MOUSEMOVE if userdata.entered => {
                    let cursor_moved = userdata.last_position != Some(position);
                    userdata.last_position = Some(position);
                    if cursor_moved {
                        unsafe {
                            SetTimer(hwnd, WM_USER_LEAVE_TIMER_ID as _, 15, Some(tray_timer_proc))
                        };

                        TrayIconEvent::Move { id, rect, position }
                    } else {
                        return 0;
                    }
                }

                _ => unreachable!(),
            };

            TrayIconEvent::send(event);
        }

        WM_TIMER if wparam as u32 == WM_USER_LEAVE_TIMER_ID => {
            if let Some(position) = userdata.last_position.take() {
                let mut cursor = POINT { x: 0, y: 0 };
                if unsafe { GetCursorPos(&mut cursor as _) } == 0 {
                    return 0;
                }

                let rect = match get_tray_rect(userdata.internal_id, hwnd) {
                    Some(r) => r,
                    None => return 0,
                };

                let in_x = (rect.left..rect.right).contains(&cursor.x);
                let in_y = (rect.top..rect.bottom).contains(&cursor.y);

                if !in_x || !in_y {
                    unsafe { KillTimer(hwnd, WM_USER_LEAVE_TIMER_ID as _) };
                    userdata.entered = false;

                    TrayIconEvent::send(TrayIconEvent::Leave {
                        id: userdata.id.clone(),
                        rect: rect.into(),
                        position,
                    });
                }
            }

            return 0;
        }

        _ => {}
    }

    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

unsafe extern "system" fn tray_timer_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: u32) {
    unsafe { tray_proc(hwnd, msg, wparam, lparam as _) };
}

#[inline]
unsafe fn register_tray_icon(
    hwnd: HWND,
    tray_id: u32,
    hicon: &Option<HICON>,
    tooltip: &Option<String>,
) -> bool {
    let mut h_icon = ptr::null_mut();
    let mut flags = NIF_MESSAGE;
    let mut sz_tip: [u16; 128] = [0; 128];

    if let Some(hicon) = hicon {
        flags |= NIF_ICON;
        h_icon = *hicon;
    }

    if let Some(tooltip) = tooltip {
        flags |= NIF_TIP;
        let tip = encode_wide(tooltip);
        #[allow(clippy::manual_memcpy)]
        for i in 0..tip.len().min(128) {
            sz_tip[i] = tip[i];
        }
    }

    let mut nid = NOTIFYICONDATAW {
        uFlags: flags,
        hWnd: hwnd,
        uID: tray_id,
        uCallbackMessage: WM_USER_TRAYICON,
        hIcon: h_icon,
        szTip: sz_tip,
        ..unsafe { std::mem::zeroed() }
    };

    unsafe { Shell_NotifyIconW(NIM_ADD, &mut nid as _) == TRUE }
}

#[inline]
unsafe fn remove_tray_icon(hwnd: HWND, id: u32) {
    let mut nid = NOTIFYICONDATAW {
        uFlags: NIF_ICON,
        hWnd: hwnd,
        uID: id,
        ..unsafe { std::mem::zeroed() }
    };

    let _ = unsafe { Shell_NotifyIconW(NIM_DELETE, &mut nid as _) };
}

#[inline]
fn get_tray_rect(id: u32, hwnd: HWND) -> Option<RECT> {
    let nid = NOTIFYICONIDENTIFIER {
        hWnd: hwnd,
        cbSize: std::mem::size_of::<NOTIFYICONIDENTIFIER>() as _,
        uID: id,
        ..unsafe { std::mem::zeroed() }
    };

    let mut rect = RECT {
        left: 0,
        bottom: 0,
        right: 0,
        top: 0,
    };
    if unsafe { Shell_NotifyIconGetRect(&nid, &mut rect) } == S_OK {
        Some(rect)
    } else {
        None
    }
}

impl From<RECT> for Rect {
    fn from(rect: RECT) -> Self {
        Self {
            position: dpi::PhysicalPosition::new(rect.left.into(), rect.top.into()),
            size: dpi::PhysicalSize::new(
                rect.right.saturating_sub(rect.left) as u32,
                rect.bottom.saturating_sub(rect.top) as u32,
            ),
        }
    }
}
