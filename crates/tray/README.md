# tray

Cross-platform system tray icon library for Rust.

## Features

- **Native implementations**: X11 (Linux), Shell_NotifyIcon (Windows), NSStatusItem (macOS)
- **Full event support**: Click, double-click, enter, leave, move
- **Thread-safe**: `TrayIcon` is `Send + Sync` for use with async runtimes

## Installation

```bash
cargo add tray
```

## Usage

```rust
use tray::{Icon, TrayIconBuilder, TrayIconEvent, MouseButton};

let icon = Icon::from_rgba(rgba_data, 32, 32)?;

let tray = TrayIconBuilder::new()
    .with_icon(icon)
    .with_tooltip("My App")
    .build()?;

// Poll for events
let receiver = TrayIconEvent::receiver();
loop {
    if let Ok(event) = receiver.try_recv() {
        match event {
            TrayIconEvent::Click { button: MouseButton::Right, position, .. } => {
                // Show your own menu/popup at `position`
            }
            _ => {}
        }
    }
    std::thread::sleep(std::time::Duration::from_millis(100));
}
```

## Feature Flags

| Feature | Description |
|---------|-------------|
| `serde` | Serialize/deserialize events and IDs |

## Platform Notes

- **Linux**: Uses native X11 system tray protocol.
- **Windows**: Uses `Shell_NotifyIconW`. Handles taskbar restart automatically.
- **macOS**: Uses `NSStatusItem`. Requires main thread (`MainThreadMarker`).

## License

MIT
