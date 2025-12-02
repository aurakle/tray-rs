//! Popup menus for tray icons.
//!
//! # Example
//!
//! ```no_run
//! use tray_menu::{PopupMenu, TextEntry, Divider, TrayIconEvent, MouseButton, MouseButtonState};
//!
//! let tray = tray_menu::TrayIconBuilder::new().build().unwrap();
//! let receiver = TrayIconEvent::receiver();
//!
//! loop {
//!     if let Ok(event) = receiver.try_recv() {
//!         if let TrayIconEvent::Click {
//!             button: MouseButton::Right,
//!             button_state: MouseButtonState::Up,
//!             position, ..
//!         } = event {
//!             let mut menu = PopupMenu::new();
//!             menu.add(&TextEntry::of("quit", "Quit"));
//!             if let Some(id) = menu.popup(position) {
//!                 if id.0 == "quit" { break; }
//!             }
//!         }
//!     }
//!     std::thread::sleep(std::time::Duration::from_millis(16));
//! }
//! ```

mod entry;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "macos")]
mod macos;

pub use entry::{AsEntry, CheckEntry, Divider, EntryKind, ItemId, PopupMenu, SubMenu, TextEntry};

#[cfg(target_os = "linux")]
pub use linux::{set_backend, Backend};

pub use tray::{
    dpi, BadIcon, Error, Icon, MouseButton, MouseButtonState, Rect, Result, TrayIcon,
    TrayIconAttributes, TrayIconBuilder, TrayIconEvent, TrayIconEventReceiver, TrayIconId,
};

#[cfg(target_os = "macos")]
pub use tray::NativeIcon;

use dpi::PhysicalPosition;

impl PopupMenu {
    /// Displays the menu at the given screen position and waits for selection.
    ///
    /// Returns the [`ItemId`] of the selected item, or `None` if the menu was
    /// dismissed without selection.
    ///
    /// This method blocks until the user selects an item or dismisses the menu.
    #[cfg(target_os = "linux")]
    pub fn popup(&self, position: PhysicalPosition<f64>) -> Option<ItemId> {
        linux::popup(self, position)
    }

    /// Displays the menu at the given screen position and waits for selection.
    ///
    /// Returns the [`ItemId`] of the selected item, or `None` if the menu was
    /// dismissed without selection.
    ///
    /// This method blocks until the user selects an item or dismisses the menu.
    #[cfg(target_os = "windows")]
    pub fn popup(&self, position: PhysicalPosition<f64>) -> Option<ItemId> {
        windows::popup(self, position)
    }

    /// Displays the menu at the given screen position and waits for selection.
    ///
    /// Returns the [`ItemId`] of the selected item, or `None` if the menu was
    /// dismissed without selection.
    ///
    /// This method blocks until the user selects an item or dismisses the menu.
    #[cfg(target_os = "macos")]
    pub fn popup(&self, position: PhysicalPosition<f64>) -> Option<ItemId> {
        macos::popup(self, position)
    }

    /// Displays the menu at the given screen position and waits for selection.
    ///
    /// Returns the [`ItemId`] of the selected item, or `None` if the menu was
    /// dismissed without selection.
    ///
    /// This method blocks until the user selects an item or dismisses the menu.
    #[cfg(all(
        not(target_os = "linux"),
        not(target_os = "windows"),
        not(target_os = "macos")
    ))]
    pub fn popup(&self, _position: PhysicalPosition<f64>) -> Option<ItemId> {
        None
    }
}
