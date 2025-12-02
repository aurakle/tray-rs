//! Example demonstrating how to spawn an egui popup window on tray icon click.
//!
//! This example shows the pattern for integrating tray icons with egui/eframe applications.
//! When the tray icon is clicked, a popup window appears at the cursor position.
//!
//! Run with: cargo run -p tray --example egui-popup

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use eframe::egui;
use tray::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

fn main() -> eframe::Result {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/icon.png");
    let icon = load_icon(std::path::Path::new(path));

    let tray_icon = TrayIconBuilder::new()
        .with_tooltip("Egui Popup Example")
        .with_icon(icon)
        .build()
        .expect("Failed to create tray icon");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1.0, 1.0])
            .with_position([-1000.0, -1000.0])
            .with_decorations(false)
            .with_transparent(true)
            .with_visible(false),
        ..Default::default()
    };

    eframe::run_native(
        "Egui Tray Popup",
        options,
        Box::new(|_cc| Ok(Box::new(MyApp::new(tray_icon)))),
    )
}

struct MyApp {
    _tray_icon: TrayIcon,
    show_popup: Arc<AtomicBool>,
    popup_position: Arc<std::sync::Mutex<egui::Pos2>>,
    counter: i32,
}

impl MyApp {
    fn new(tray_icon: TrayIcon) -> Self {
        Self {
            _tray_icon: tray_icon,
            show_popup: Arc::new(AtomicBool::new(false)),
            popup_position: Arc::new(std::sync::Mutex::new(egui::Pos2::ZERO)),
            counter: 0,
        }
    }
}

impl eframe::App for MyApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(-1000.0, -1000.0)));

        while let Ok(event) = TrayIconEvent::receiver().try_recv() {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                position,
                ..
            } = event
            {
                self.show_popup.store(true, Ordering::Relaxed);
                let mut pos = self.popup_position.lock().unwrap();
                *pos = egui::Pos2::new(position.x as f32, position.y as f32);
            }
        }

        if self.show_popup.load(Ordering::Relaxed) {
            let show_popup = self.show_popup.clone();
            let position = *self.popup_position.lock().unwrap();

            ctx.show_viewport_immediate(
                egui::ViewportId::from_hash_of("popup"),
                egui::ViewportBuilder::default()
                    .with_title("Popup")
                    .with_inner_size([200.0, 150.0])
                    .with_position(position)
                    .with_decorations(false)
                    .with_resizable(false)
                    .with_always_on_top()
                    .with_window_type(egui::X11WindowType::Notification),
                |ctx, _class| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        ui.heading("Tray Popup");
                        ui.label("This window appeared at the tray icon click position.");

                        if ui.button("Increment Counter").clicked() {
                            self.counter += 1;
                        }

                        ui.separator();

                        if ui.button("Close").clicked() {
                            show_popup.store(false, Ordering::Relaxed);
                        }
                    });

                    if ctx.input(|i| i.viewport().close_requested()) {
                        show_popup.store(false, Ordering::Relaxed);
                    }
                },
            );
        }

        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}

fn load_icon(path: &std::path::Path) -> Icon {
    let (icon_rgba, icon_width, icon_height) = {
        let image = image::open(path)
            .expect("Failed to open icon path")
            .into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };
    Icon::from_rgba(icon_rgba, icon_width, icon_height).expect("Failed to create icon")
}
