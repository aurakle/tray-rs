use std::time::Duration;
use tray_menu::{
    CheckEntry, Divider, Icon, MouseButton, MouseButtonState, PopupMenu, SubMenu, TextEntry,
    TrayIconBuilder, TrayIconEvent,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Trigger {
    #[default]
    Right,
    Left,
    Middle,
    Enter,
}

fn parse_trigger() -> Trigger {
    std::env::args().nth(1).map_or(Trigger::default(), |s| match s.as_str() {
        "left" => Trigger::Left,
        "right" => Trigger::Right,
        "middle" => Trigger::Middle,
        "enter" => Trigger::Enter,
        _ => {
            eprintln!("Unknown trigger: {s}. Use: left, right, middle, enter");
            std::process::exit(1);
        }
    })
}

fn main() {
    #[cfg(all(target_os = "linux", feature = "gtk"))]
    gtk::init().unwrap();

    let trigger = parse_trigger();

    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../tray/examples/icon.png");
    let icon = load_icon(std::path::Path::new(path));

    let _tray_icon = TrayIconBuilder::new()
        .with_tooltip("Tray Menu Example")
        .with_icon(icon)
        .build()
        .unwrap();

    let tray_channel = TrayIconEvent::receiver();

    let trigger_desc = match trigger {
        Trigger::Right => "Right-click",
        Trigger::Left => "Left-click",
        Trigger::Middle => "Middle-click",
        Trigger::Enter => "Hover over",
    };
    println!("Tray icon created. {trigger_desc} to see menu. Ctrl+C to exit.");

    'main: loop {
        while let Ok(event) = tray_channel.try_recv() {
            println!("Tray event: {event:?}");

            let position = match (&event, trigger) {
                (TrayIconEvent::Click { button, button_state: MouseButtonState::Up, position, .. }, Trigger::Left) if *button == MouseButton::Left => Some(*position),
                (TrayIconEvent::Click { button, button_state: MouseButtonState::Up, position, .. }, Trigger::Right) if *button == MouseButton::Right => Some(*position),
                (TrayIconEvent::Click { button, button_state: MouseButtonState::Up, position, .. }, Trigger::Middle) if *button == MouseButton::Middle => Some(*position),
                (TrayIconEvent::Enter { position, .. }, Trigger::Enter) => Some(*position),
                _ => None,
            };

            if let Some(pos) = position
                && let Some(id) = show_menu(pos)
            {
                println!("Menu item selected: {id:?}");
                if id.0 == "quit" {
                    break 'main;
                }
            }
        }
        std::thread::sleep(Duration::from_millis(16));
    }
}

fn show_menu(position: tray_menu::dpi::PhysicalPosition<f64>) -> Option<tray_menu::ItemId> {
    let mut menu = PopupMenu::new();

    menu.add(&TextEntry::of("show", "Show Window"));
    menu.add(&TextEntry::of("settings", "Settings"));
    menu.add(&Divider);

    let mut help = SubMenu::of("Help");
    help.add(&TextEntry::of("about", "About"));
    help.add(&TextEntry::of("docs", "Documentation"));
    menu.add(&help);

    menu.add(&Divider);
    menu.add(&CheckEntry::of("notifications", "Notifications", true));
    menu.add(&Divider);
    menu.add(&TextEntry::of("quit", "Quit"));

    menu.popup(position)
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
