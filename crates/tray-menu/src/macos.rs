use std::cell::Cell;

use dpi::PhysicalPosition;
use objc2::rc::Retained;
use objc2::{define_class, msg_send, sel, DeclaredClass};
use objc2_app_kit::{NSMenu, NSMenuItem};
use objc2_core_graphics::{CGDisplayPixelsHigh, CGMainDisplayID};
use objc2_foundation::{MainThreadMarker, NSObject, NSPoint, NSString};

use crate::entry::{EntryKind, ItemId, PopupMenu};

#[derive(Debug)]
struct MenuDelegateIvars {
    selected_index: Cell<Option<usize>>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "TrayMenuDelegate"]
    #[ivars = MenuDelegateIvars]
    struct MenuDelegate;

    impl MenuDelegate {
        #[unsafe(method(menuItemClicked:))]
        fn menu_item_clicked(&self, sender: &NSMenuItem) {
            let tag = sender.tag();
            self.ivars().selected_index.set(Some(tag as usize));
        }
    }
);

pub fn popup(menu: &PopupMenu, position: PhysicalPosition<f64>) -> Option<ItemId> {
    let mtm = MainThreadMarker::new()?;

    let delegate = mtm.alloc().set_ivars(MenuDelegateIvars {
        selected_index: Cell::new(None),
    });
    let delegate: Retained<MenuDelegate> = unsafe { msg_send![super(delegate), init] };

    let ns_menu = NSMenu::new(mtm);
    let mut id_map: Vec<ItemId> = Vec::new();
    build_menu(&ns_menu, menu.entries(), &delegate, &mut id_map, mtm);

    ns_menu.popUpMenuPositioningItem_atLocation_inView(
        None,
        NSPoint::new(position.x, flip_y(position.y)),
        None,
    );

    delegate
        .ivars()
        .selected_index
        .get()
        .and_then(|idx| id_map.get(idx).cloned())
}

fn build_menu(
    ns_menu: &NSMenu,
    entries: &[EntryKind],
    delegate: &MenuDelegate,
    id_map: &mut Vec<ItemId>,
    mtm: MainThreadMarker,
) {
    for entry in entries {
        match entry {
            EntryKind::Text(e) => {
                id_map.push(e.id().clone());
                let tag = (id_map.len() - 1) as isize;
                let item = unsafe {
                    NSMenuItem::initWithTitle_action_keyEquivalent(
                        mtm.alloc(),
                        &NSString::from_str(e.text()),
                        Some(sel!(menuItemClicked:)),
                        &NSString::from_str(""),
                    )
                };
                unsafe {
                    item.setTag(tag);
                    item.setTarget(Some(delegate));
                    item.setEnabled(e.active());
                }
                ns_menu.addItem(&item);
            }
            EntryKind::Check(e) => {
                id_map.push(e.id().clone());
                let tag = (id_map.len() - 1) as isize;
                let item = unsafe {
                    NSMenuItem::initWithTitle_action_keyEquivalent(
                        mtm.alloc(),
                        &NSString::from_str(e.text()),
                        Some(sel!(menuItemClicked:)),
                        &NSString::from_str(""),
                    )
                };
                unsafe {
                    item.setTag(tag);
                    item.setTarget(Some(delegate));
                    item.setState(if e.ticked() { 1 } else { 0 });
                    item.setEnabled(e.active());
                }
                ns_menu.addItem(&item);
            }
            EntryKind::Sub(sub) => {
                let item = unsafe {
                    NSMenuItem::initWithTitle_action_keyEquivalent(
                        mtm.alloc(),
                        &NSString::from_str(sub.text()),
                        None,
                        &NSString::from_str(""),
                    )
                };
                let submenu = NSMenu::new(mtm);
                build_menu(&submenu, sub.entries(), delegate, id_map, mtm);
                item.setSubmenu(Some(&submenu));
                item.setEnabled(sub.active());
                ns_menu.addItem(&item);
            }
            EntryKind::Divider => {
                ns_menu.addItem(&NSMenuItem::separatorItem(mtm));
            }
        }
    }
}

fn flip_y(y: f64) -> f64 {
    CGDisplayPixelsHigh(CGMainDisplayID()) as f64 - y
}
