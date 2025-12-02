use dpi::PhysicalPosition;

use crate::entry::{ItemId, PopupMenu};

/// The menu rendering backend to use on Linux.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Backend {
    #[default]
    Gtk,
    Qt,
}

static PREFERRED_BACKEND: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// Sets the preferred menu backend for Linux.
///
/// Call this before displaying any menus. If both `gtk` and `qt` features are
/// enabled, this determines which backend is used. Defaults to GTK.
pub fn set_backend(backend: Backend) {
    PREFERRED_BACKEND.store(backend as u8, std::sync::atomic::Ordering::Relaxed);
}

#[cfg(any(feature = "gtk", feature = "qt"))]
fn preferred_backend() -> Backend {
    match PREFERRED_BACKEND.load(std::sync::atomic::Ordering::Relaxed) {
        1 => Backend::Qt,
        _ => Backend::Gtk,
    }
}

#[cfg(any(feature = "gtk", feature = "qt"))]
pub fn popup(menu: &PopupMenu, position: PhysicalPosition<f64>) -> Option<ItemId> {
    match preferred_backend() {
        Backend::Gtk => {
            #[cfg(feature = "gtk")]
            { gtk_backend::popup(menu, position) }
            #[cfg(all(not(feature = "gtk"), feature = "qt"))]
            { qt_backend::popup(menu, position) }
        }
        Backend::Qt => {
            #[cfg(feature = "qt")]
            { qt_backend::popup(menu, position) }
            #[cfg(all(not(feature = "qt"), feature = "gtk"))]
            { gtk_backend::popup(menu, position) }
        }
    }
}

#[cfg(all(not(feature = "gtk"), not(feature = "qt")))]
pub fn popup(_menu: &PopupMenu, _position: PhysicalPosition<f64>) -> Option<ItemId> {
    None
}

#[cfg(feature = "gtk")]
mod gtk_backend {
    use std::cell::RefCell;
    use std::rc::Rc;

    use dpi::PhysicalPosition;
    use gtk::prelude::*;

    use crate::entry::{EntryKind, ItemId, PopupMenu};

    pub fn popup(menu: &PopupMenu, position: PhysicalPosition<f64>) -> Option<ItemId> {
        if !gtk::is_initialized() {
            gtk::init().ok()?;
        }

        let selected: Rc<RefCell<Option<ItemId>>> = Rc::new(RefCell::new(None));
        let gtk_menu = gtk::Menu::new();

        build_menu(&gtk_menu, menu.entries(), &selected);

        gtk_menu.show_all();

        let done = Rc::new(RefCell::new(false));
        let done_clone = Rc::clone(&done);

        gtk_menu.connect_hide(move |_| {
            *done_clone.borrow_mut() = true;
        });

        gtk_menu.popup_at_rect(
            &gtk::gdk::Display::default()
                .expect("No display")
                .default_seat()
                .expect("No seat")
                .pointer()
                .expect("No pointer")
                .display()
                .default_screen()
                .root_window()
                .expect("No root window"),
            &gtk::gdk::Rectangle::new(position.x as i32, position.y as i32, 1, 1),
            gtk::gdk::Gravity::NorthWest,
            gtk::gdk::Gravity::NorthWest,
            None,
        );

        while !*done.borrow() {
            if gtk::events_pending() {
                gtk::main_iteration();
            } else {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }

        selected.borrow().clone()
    }

    fn build_menu(
        gtk_menu: &gtk::Menu,
        entries: &[EntryKind],
        selected: &Rc<RefCell<Option<ItemId>>>,
    ) {
        for entry in entries {
            match entry {
                EntryKind::Text(e) => {
                    let gtk_item = gtk::MenuItem::with_label(e.text());
                    gtk_item.set_sensitive(e.active());

                    let id = e.id().clone();
                    let selected = Rc::clone(selected);
                    gtk_item.connect_activate(move |_| {
                        *selected.borrow_mut() = Some(id.clone());
                    });

                    gtk_menu.append(&gtk_item);
                }
                EntryKind::Check(e) => {
                    let gtk_item = gtk::CheckMenuItem::with_label(e.text());
                    gtk_item.set_active(e.ticked());
                    gtk_item.set_sensitive(e.active());

                    let id = e.id().clone();
                    let selected = Rc::clone(selected);
                    gtk_item.connect_activate(move |_| {
                        *selected.borrow_mut() = Some(id.clone());
                    });

                    gtk_menu.append(&gtk_item);
                }
                EntryKind::Sub(sub) => {
                    let gtk_item = gtk::MenuItem::with_label(sub.text());
                    gtk_item.set_sensitive(sub.active());

                    let submenu = gtk::Menu::new();
                    build_menu(&submenu, sub.entries(), selected);
                    gtk_item.set_submenu(Some(&submenu));

                    gtk_menu.append(&gtk_item);
                }
                EntryKind::Divider => {
                    let gtk_item = gtk::SeparatorMenuItem::new();
                    gtk_menu.append(&gtk_item);
                }
            }
        }
    }
}

#[cfg(feature = "qt")]
mod qt_backend {
    use cxx_qt_lib::{QPoint, QString};
    use dpi::PhysicalPosition;

    use crate::entry::{EntryKind, ItemId, PopupMenu};

    #[cxx::bridge]
    mod ffi {
        unsafe extern "C++" {
            include!("cxx-qt-lib/qpoint.h");
            type QPoint = cxx_qt_lib::QPoint;

            include!("cxx-qt-lib/qstring.h");
            type QString = cxx_qt_lib::QString;
        }

        unsafe extern "C++" {
            include!("tray-menu/qt_helpers.h");

            type QMenu;
            type QAction;
        }

        #[namespace = "tray_menu_qt"]
        unsafe extern "C++" {
            fn ensure_qapplication() -> bool;
            fn create_menu() -> *mut QMenu;
            unsafe fn delete_menu(menu: *mut QMenu);
            unsafe fn add_action(menu: *mut QMenu, text: &QString) -> *mut QAction;
            unsafe fn add_separator(menu: *mut QMenu);
            unsafe fn add_submenu(menu: *mut QMenu, title: &QString) -> *mut QMenu;
            unsafe fn set_action_enabled(action: *mut QAction, enabled: bool);
            unsafe fn set_action_checkable(action: *mut QAction, checkable: bool);
            unsafe fn set_action_checked(action: *mut QAction, checked: bool);
            unsafe fn set_action_data(action: *mut QAction, index: i32);
            unsafe fn get_action_data(action: *const QAction) -> i32;
            unsafe fn exec_menu(menu: *mut QMenu, pos: &QPoint) -> *mut QAction;
        }
    }

    pub fn popup(menu: &PopupMenu, position: PhysicalPosition<f64>) -> Option<ItemId> {
        if !ffi::ensure_qapplication() {
            return None;
        }

        let mut id_map: Vec<ItemId> = Vec::new();

        let qt_menu = ffi::create_menu();
        if qt_menu.is_null() {
            return None;
        }

        unsafe {
            build_menu(qt_menu, menu.entries(), &mut id_map);

            let pos = QPoint::new(position.x as i32, position.y as i32);
            let selected_action = ffi::exec_menu(qt_menu, &pos);

            let result = if selected_action.is_null() {
                None
            } else {
                let index = ffi::get_action_data(selected_action);
                if index >= 0 && (index as usize) < id_map.len() {
                    Some(id_map[index as usize].clone())
                } else {
                    None
                }
            };

            ffi::delete_menu(qt_menu);

            result
        }
    }

    unsafe fn build_menu(qt_menu: *mut ffi::QMenu, entries: &[EntryKind], id_map: &mut Vec<ItemId>) {
        unsafe {
            for entry in entries {
                match entry {
                    EntryKind::Text(e) => {
                        let text = QString::from(e.text());
                        let action = ffi::add_action(qt_menu, &text);
                        if !action.is_null() {
                            ffi::set_action_enabled(action, e.active());
                            ffi::set_action_data(action, id_map.len() as i32);
                            id_map.push(e.id().clone());
                        }
                    }
                    EntryKind::Check(e) => {
                        let text = QString::from(e.text());
                        let action = ffi::add_action(qt_menu, &text);
                        if !action.is_null() {
                            ffi::set_action_checkable(action, true);
                            ffi::set_action_checked(action, e.ticked());
                            ffi::set_action_enabled(action, e.active());
                            ffi::set_action_data(action, id_map.len() as i32);
                            id_map.push(e.id().clone());
                        }
                    }
                    EntryKind::Sub(sub) => {
                        let title = QString::from(sub.text());
                        let submenu = ffi::add_submenu(qt_menu, &title);
                        if !submenu.is_null() {
                            build_menu(submenu, sub.entries(), id_map);
                        }
                    }
                    EntryKind::Divider => {
                        ffi::add_separator(qt_menu);
                    }
                }
            }
        }
    }
}
