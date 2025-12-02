use std::io::Cursor;

use objc2::rc::Retained;
use objc2::Message;
use objc2::{define_class, msg_send, AllocAnyThread, DeclaredClass};
use objc2_app_kit::{
    NSCellImagePosition, NSEvent, NSImage, NSStatusBar, NSStatusItem, NSTrackingArea,
    NSTrackingAreaOptions, NSVariableStatusItemLength, NSView, NSWindow,
};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_core_graphics::{CGDisplayPixelsHigh, CGMainDisplayID};
use objc2_foundation::{MainThreadMarker, NSData, NSSize, NSString};

use crate::{
    icon::{BadIcon, Icon},
    tray::{MouseButton, MouseButtonState, Rect, TrayIconAttributes, TrayIconEvent, TrayIconId},
    Error,
};

#[derive(Debug, Clone)]
pub struct PlatformIcon {
    rgba: Vec<u8>,
    width: u32,
    height: u32,
}

impl PlatformIcon {
    pub fn from_rgba(rgba: Vec<u8>, width: u32, height: u32) -> Result<Self, BadIcon> {
        Ok(Self {
            rgba,
            width,
            height,
        })
    }

    pub fn get_size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    pub fn to_png(&self) -> crate::Result<Vec<u8>> {
        let mut png_data = Vec::new();
        {
            let mut encoder = png::Encoder::new(Cursor::new(&mut png_data), self.width, self.height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header()?;
            writer.write_image_data(&self.rgba)?;
        }
        Ok(png_data)
    }
}

pub struct TrayIconImpl {
    ns_status_item: Option<Retained<NSStatusItem>>,
    tray_target: Option<Retained<TrayTarget>>,
    id: TrayIconId,
    attrs: TrayIconAttributes,
    mtm: MainThreadMarker,
}

impl TrayIconImpl {
    pub fn new(id: TrayIconId, attrs: TrayIconAttributes) -> crate::Result<Self> {
        let mtm = MainThreadMarker::new().ok_or(Error::NotMainThread)?;
        let (ns_status_item, tray_target) = Self::create(&id, &attrs, mtm)?;

        let tray_icon = Self {
            ns_status_item: Some(ns_status_item),
            tray_target: Some(tray_target),
            id,
            attrs,
            mtm,
        };

        Ok(tray_icon)
    }

    fn create(
        id: &TrayIconId,
        attrs: &TrayIconAttributes,
        mtm: MainThreadMarker,
    ) -> crate::Result<(Retained<NSStatusItem>, Retained<TrayTarget>)> {
        let ns_status_item =
            NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength);

        set_icon_for_ns_status_item_button(
            &ns_status_item,
            attrs.icon.clone(),
            attrs.icon_is_template,
            mtm,
        )?;

        Self::set_tooltip_inner(&ns_status_item, attrs.tooltip.clone(), mtm)?;
        Self::set_title_inner(&ns_status_item, attrs.title.clone(), mtm);

        let tray_target = {
            let button = ns_status_item.button(mtm).unwrap();

            let frame = button.frame();

            let target = mtm.alloc().set_ivars(TrayTargetIvars {
                id: NSString::from_str(&id.0),
                status_item: ns_status_item.retain(),
            });
            let tray_target: Retained<TrayTarget> =
                unsafe { msg_send![super(target), initWithFrame: frame] };
            tray_target.setWantsLayer(true);

            button.addSubview(&tray_target);

            tray_target
        };

        Ok((ns_status_item, tray_target))
    }

    fn remove(&mut self) {
        if let (Some(ns_status_item), Some(tray_target)) =
            (&self.ns_status_item, &self.tray_target)
        {
            NSStatusBar::systemStatusBar().removeStatusItem(ns_status_item);
            tray_target.removeFromSuperview();
        }

        self.ns_status_item = None;
        self.tray_target = None;
    }

    pub fn set_icon(&mut self, icon: Option<Icon>) -> crate::Result<()> {
        if let (Some(ns_status_item), Some(tray_target)) =
            (&self.ns_status_item, &self.tray_target)
        {
            set_icon_for_ns_status_item_button(ns_status_item, icon.clone(), false, self.mtm)?;
            tray_target.update_dimensions();
        }
        self.attrs.icon = icon;
        Ok(())
    }

    pub fn set_tooltip<S: AsRef<str>>(&mut self, tooltip: Option<S>) -> crate::Result<()> {
        let tooltip = tooltip.map(|s| s.as_ref().to_string());
        if let (Some(ns_status_item), Some(tray_target)) =
            (&self.ns_status_item, &self.tray_target)
        {
            Self::set_tooltip_inner(ns_status_item, tooltip.clone(), self.mtm)?;
            tray_target.update_dimensions();
        }
        self.attrs.tooltip = tooltip;
        Ok(())
    }

    fn set_tooltip_inner<S: AsRef<str>>(
        ns_status_item: &NSStatusItem,
        tooltip: Option<S>,
        mtm: MainThreadMarker,
    ) -> crate::Result<()> {
        let tooltip = tooltip.map(|tooltip| NSString::from_str(tooltip.as_ref()));
        if let Some(button) = ns_status_item.button(mtm) {
            button.setToolTip(tooltip.as_deref());
        }
        Ok(())
    }

    pub fn set_title<S: AsRef<str>>(&mut self, title: Option<S>) {
        let title = title.map(|s| s.as_ref().to_string());
        if let (Some(ns_status_item), Some(tray_target)) =
            (&self.ns_status_item, &self.tray_target)
        {
            Self::set_title_inner(ns_status_item, title.clone(), self.mtm);
            tray_target.update_dimensions();
        }
        self.attrs.title = title;
    }

    fn set_title_inner<S: AsRef<str>>(
        ns_status_item: &NSStatusItem,
        title: Option<S>,
        mtm: MainThreadMarker,
    ) {
        if let Some(title) = title {
            if let Some(button) = ns_status_item.button(mtm) {
                button.setTitle(&NSString::from_str(title.as_ref()));
            }
        }
    }

    pub fn set_visible(&mut self, visible: bool) -> crate::Result<()> {
        if visible {
            if self.ns_status_item.is_none() {
                let (ns_status_item, tray_target) = Self::create(&self.id, &self.attrs, self.mtm)?;
                self.ns_status_item = Some(ns_status_item);
                self.tray_target = Some(tray_target);
            }
        } else {
            self.remove();
        }

        Ok(())
    }

    pub fn set_icon_as_template(&mut self, is_template: bool) {
        if let Some(ns_status_item) = &self.ns_status_item {
            let button = ns_status_item.button(self.mtm).unwrap();
            if let Some(nsimage) = button.image() {
                nsimage.setTemplate(is_template);
                button.setImage(Some(&nsimage));
            }
        }
        self.attrs.icon_is_template = is_template;
    }

    pub fn set_icon_with_as_template(
        &mut self,
        icon: Option<Icon>,
        is_template: bool,
    ) -> crate::Result<()> {
        if let (Some(ns_status_item), Some(tray_target)) =
            (&self.ns_status_item, &self.tray_target)
        {
            set_icon_for_ns_status_item_button(
                ns_status_item,
                icon.clone(),
                is_template,
                self.mtm,
            )?;
            tray_target.update_dimensions();
        }
        self.attrs.icon = icon;
        self.attrs.icon_is_template = is_template;
        Ok(())
    }

    pub fn rect(&self) -> Option<Rect> {
        let ns_status_item = self.ns_status_item.as_deref()?;
        let button = ns_status_item.button(self.mtm).unwrap();
        let window = button.window();
        window.map(|window| get_tray_rect(&window))
    }
}

impl Drop for TrayIconImpl {
    fn drop(&mut self) {
        self.remove()
    }
}

fn set_icon_for_ns_status_item_button(
    ns_status_item: &NSStatusItem,
    icon: Option<Icon>,
    icon_is_template: bool,
    mtm: MainThreadMarker,
) -> crate::Result<()> {
    let button = ns_status_item.button(mtm).unwrap();

    if let Some(icon) = icon {
        let png_icon = icon.inner.to_png()?;

        let (width, height) = icon.inner.get_size();

        let icon_height: f64 = 18.0;
        let icon_width: f64 = (width as f64) / (height as f64 / icon_height);

        let nsdata = NSData::from_vec(png_icon);

        let nsimage = NSImage::initWithData(NSImage::alloc(), &nsdata).unwrap();
        let new_size = NSSize::new(icon_width, icon_height);

        button.setImage(Some(&nsimage));
        nsimage.setSize(new_size);
        button.setImagePosition(NSCellImagePosition::ImageLeft);
        nsimage.setTemplate(icon_is_template);
    } else {
        button.setImage(None);
    }

    Ok(())
}

#[derive(Debug)]
struct TrayTargetIvars {
    id: Retained<NSString>,
    status_item: Retained<NSStatusItem>,
}

define_class!(
    #[unsafe(super(NSView))]
    #[name = "TaoTrayTarget"]
    #[ivars = TrayTargetIvars]
    struct TrayTarget;

    impl TrayTarget {
        #[unsafe(method(mouseDown:))]
        fn on_mouse_down(&self, event: &NSEvent) {
            send_mouse_event(
                self,
                event,
                MouseEventType::Click,
                Some(MouseClickEvent {
                    button: MouseButton::Left,
                    state: MouseButtonState::Down,
                }),
            );
            on_tray_click(self, MouseButton::Left);
        }

        #[unsafe(method(mouseUp:))]
        fn on_mouse_up(&self, event: &NSEvent) {
            let mtm = MainThreadMarker::from(self);
            let button = self.ivars().status_item.button(mtm).unwrap();
            button.highlight(false);
            send_mouse_event(
                self,
                event,
                MouseEventType::Click,
                Some(MouseClickEvent {
                    button: MouseButton::Left,
                    state: MouseButtonState::Up,
                }),
            );
        }

        #[unsafe(method(rightMouseDown:))]
        fn on_right_mouse_down(&self, event: &NSEvent) {
            send_mouse_event(
                self,
                event,
                MouseEventType::Click,
                Some(MouseClickEvent {
                    button: MouseButton::Right,
                    state: MouseButtonState::Down,
                }),
            );
            on_tray_click(self, MouseButton::Right);
        }

        #[unsafe(method(rightMouseUp:))]
        fn on_right_mouse_up(&self, event: &NSEvent) {
            send_mouse_event(
                self,
                event,
                MouseEventType::Click,
                Some(MouseClickEvent {
                    button: MouseButton::Right,
                    state: MouseButtonState::Up,
                }),
            );
        }

        #[unsafe(method(otherMouseDown:))]
        fn on_other_mouse_down(&self, event: &NSEvent) {
            let button_number = event.buttonNumber();
            if button_number == 2 {
                send_mouse_event(
                    self,
                    event,
                    MouseEventType::Click,
                    Some(MouseClickEvent {
                        button: MouseButton::Middle,
                        state: MouseButtonState::Down,
                    }),
                );
            }
        }

        #[unsafe(method(otherMouseUp:))]
        fn on_other_mouse_up(&self, event: &NSEvent) {
            let button_number = event.buttonNumber();
            if button_number == 2 {
                send_mouse_event(
                    self,
                    event,
                    MouseEventType::Click,
                    Some(MouseClickEvent {
                        button: MouseButton::Middle,
                        state: MouseButtonState::Up,
                    }),
                );
            }
        }

        #[unsafe(method(mouseEntered:))]
        fn on_mouse_entered(&self, event: &NSEvent) {
            send_mouse_event(self, event, MouseEventType::Enter, None);
        }

        #[unsafe(method(mouseExited:))]
        fn on_mouse_exited(&self, event: &NSEvent) {
            send_mouse_event(self, event, MouseEventType::Leave, None);
        }

        #[unsafe(method(mouseMoved:))]
        fn on_mouse_moved(&self, event: &NSEvent) {
            send_mouse_event(self, event, MouseEventType::Move, None);
        }
    }

    impl TrayTarget {
        #[unsafe(method(updateTrackingAreas))]
        fn update_tracking_areas(&self) {
            let areas = self.trackingAreas();
            for area in areas {
                self.removeTrackingArea(&area);
            }

            let _: () = unsafe { msg_send![super(self), updateTrackingAreas] };

            let options = NSTrackingAreaOptions::MouseEnteredAndExited
                | NSTrackingAreaOptions::MouseMoved
                | NSTrackingAreaOptions::ActiveAlways
                | NSTrackingAreaOptions::InVisibleRect;
            let rect = CGRect {
                origin: CGPoint { x: 0.0, y: 0.0 },
                size: CGSize {
                    width: 0.0,
                    height: 0.0,
                },
            };
            let area = unsafe {
                NSTrackingArea::initWithRect_options_owner_userInfo(
                    NSTrackingArea::alloc(),
                    rect,
                    options,
                    Some(self),
                    None,
                )
            };
            self.addTrackingArea(&area);
        }
    }
);

impl TrayTarget {
    fn update_dimensions(&self) {
        let mtm = MainThreadMarker::from(self);
        let button = self.ivars().status_item.button(mtm).unwrap();
        self.setFrame(button.frame());
    }
}

fn on_tray_click(this: &TrayTarget, button: MouseButton) {
    let mtm = MainThreadMarker::from(this);
    let ns_button = this.ivars().status_item.button(mtm).unwrap();
    if button == MouseButton::Left || button == MouseButton::Right {
        ns_button.highlight(true);
    }
}

fn get_tray_rect(window: &NSWindow) -> Rect {
    let frame = window.frame();
    let scale_factor = window.backingScaleFactor();

    Rect {
        size: dpi::LogicalSize::new(frame.size.width, frame.size.height).to_physical(scale_factor),
        position: dpi::LogicalPosition::new(
            frame.origin.x,
            flip_window_screen_coordinates(frame.origin.y) - frame.size.height,
        )
        .to_physical(scale_factor),
    }
}

fn send_mouse_event(
    this: &TrayTarget,
    event: &NSEvent,
    mouse_event_type: MouseEventType,
    click_event: Option<MouseClickEvent>,
) {
    let mtm = MainThreadMarker::from(this);
    let tray_id = TrayIconId(this.ivars().id.to_string());

    let window = event.window(mtm).unwrap();
    let icon_rect = get_tray_rect(&window);

    let mouse_location = NSEvent::mouseLocation();
    let scale_factor = window.backingScaleFactor();
    let cursor_position = dpi::LogicalPosition::new(
        mouse_location.x,
        flip_window_screen_coordinates(mouse_location.y),
    )
    .to_physical(scale_factor);

    let event = match mouse_event_type {
        MouseEventType::Click => {
            let click_event = click_event.unwrap();
            TrayIconEvent::Click {
                id: tray_id,
                position: cursor_position,
                rect: icon_rect,
                button: click_event.button,
                button_state: click_event.state,
            }
        }
        MouseEventType::Enter => TrayIconEvent::Enter {
            id: tray_id,
            position: cursor_position,
            rect: icon_rect,
        },
        MouseEventType::Leave => TrayIconEvent::Leave {
            id: tray_id,
            position: cursor_position,
            rect: icon_rect,
        },
        MouseEventType::Move => TrayIconEvent::Move {
            id: tray_id,
            position: cursor_position,
            rect: icon_rect,
        },
    };

    TrayIconEvent::send(event);
}

#[derive(Debug)]
enum MouseEventType {
    Click,
    Enter,
    Leave,
    Move,
}

#[derive(Debug)]
struct MouseClickEvent {
    button: MouseButton,
    state: MouseButtonState,
}

fn flip_window_screen_coordinates(y: f64) -> f64 {
    CGDisplayPixelsHigh(CGMainDisplayID()) as f64 - y
}
