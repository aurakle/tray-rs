//! Simple tray icon example demonstrating basic event handling.
//!
//! Run with: cargo run -p tray --example simple-tray

use std::time::Duration;
use tray::{Icon, MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

fn main() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/icon.png");
    let icon = load_icon(std::path::Path::new(path));

    let _tray_icon = TrayIconBuilder::new()
        .with_tooltip("Simple Tray Example")
        .with_icon(icon)
        .with_title("Tray")
        .build()
        .unwrap();

    let tray_channel = TrayIconEvent::receiver();

    println!("Tray icon created. Click to see events. Ctrl+C to exit.");

    loop {
        while let Ok(event) = tray_channel.try_recv() {
            println!("Tray event: {event:?}");
            if let TrayIconEvent::Click {
                button: MouseButton::Right,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                println!("Right click detected - use tray-menu crate for menus");
            }
        }
        std::thread::sleep(Duration::from_millis(16));
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
