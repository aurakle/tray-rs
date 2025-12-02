# tray-menu

Popup menus for system tray icons.

## Features

- **Simple API** for constructing popup menus
- **Native backends**: GTK (Linux), TrackPopupMenu (Windows), NSMenu (macOS)
- **Re-exports `tray`** crate for convenience

## Installation

```bash
cargo add tray-menu --features gtk  # Linux with GTK
cargo add tray-menu                  # Windows/macOS
```

## Usage

```rust
use tray_menu::{PopupMenu, TextEntry, CheckEntry, SubMenu, Divider};
use tray_menu::{TrayIconBuilder, TrayIconEvent, MouseButton, MouseButtonState};

let tray = TrayIconBuilder::new()
    .with_tooltip("My App")
    .build()?;

let receiver = TrayIconEvent::receiver();
loop {
    if let Ok(TrayIconEvent::Click { 
        button: MouseButton::Right, 
        button_state: MouseButtonState::Up,
        position, .. 
    }) = receiver.try_recv() {
        let mut menu = PopupMenu::new();
        menu.add(&TextEntry::of("show", "Show Window"));
        menu.add(&Divider);
        
        let mut help = SubMenu::of("Help");
        help.add(&TextEntry::of("about", "About"));
        menu.add(&help);
        
        menu.add(&Divider);
        menu.add(&CheckEntry::of("notify", "Notifications", true));
        menu.add(&Divider);
        menu.add(&TextEntry::of("quit", "Quit"));
        
        if let Some(id) = menu.popup(position) {
            match id.0.as_str() {
                "quit" => break,
                "show" => { /* show window */ }
                _ => {}
            }
        }
    }
    std::thread::sleep(std::time::Duration::from_millis(16));
}
```

## Menu Items

| Type | Description |
|------|-------------|
| `TextEntry::of(id, label)` | Clickable menu item |
| `CheckEntry::of(id, label, checked)` | Checkbox item |
| `SubMenu::of(label)` | Nested submenu (use `.add()` to populate) |
| `Divider` | Visual separator |

## Features

| Feature | Description |
|---------|-------------|
| `gtk` | GTK backend for Linux (optional) |
| `qt` | Qt backend for Linux (optional, requires Qt5/Qt6 dev libs) |
| `serde` | Serialize/deserialize menu types |

Note: `gtk` and `qt` features are mutually exclusive on Linux.

## Platform Backends

| Platform | Backend |
|----------|---------|
| Linux | GTK popup (`gtk` feature) or Qt popup (`qt` feature) |
| Windows | `TrackPopupMenu` (native) |
| macOS | `NSMenu` (native) |

## Qt Backend Requirements

The Qt backend requires:
- Qt5 or Qt6 development libraries

The Qt backend will automatically create a `QApplication` if one doesn't exist.

On Debian/Ubuntu:
```bash
sudo apt install qtbase5-dev  # Qt5
# or
sudo apt install qt6-base-dev  # Qt6
```

On Arch Linux:
```bash
sudo pacman -S qt5-base  # Qt5
# or
sudo pacman -S qt6-base  # Qt6
```

## License

MIT
