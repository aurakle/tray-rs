use std::ptr;

use dpi::PhysicalPosition;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, GetForegroundWindow, SetForegroundWindow,
    TrackPopupMenu, HMENU, MF_CHECKED, MF_GRAYED, MF_POPUP, MF_SEPARATOR, MF_STRING,
    TPM_LEFTALIGN, TPM_RETURNCMD, TPM_TOPALIGN,
};

use crate::entry::{EntryKind, ItemId, PopupMenu};

pub fn popup(menu: &PopupMenu, position: PhysicalPosition<f64>) -> Option<ItemId> {
    unsafe {
        let hmenu = CreatePopupMenu();
        if hmenu.is_null() {
            return None;
        }

        let mut id_map: Vec<ItemId> = Vec::new();
        build_menu(hmenu, menu.entries(), &mut id_map);

        let hwnd = GetForegroundWindow();
        SetForegroundWindow(hwnd);

        let cmd = TrackPopupMenu(
            hmenu,
            TPM_RETURNCMD | TPM_LEFTALIGN | TPM_TOPALIGN,
            position.x as i32,
            position.y as i32,
            0,
            hwnd,
            ptr::null(),
        );

        DestroyMenu(hmenu);

        if cmd > 0 {
            id_map.get((cmd - 1) as usize).cloned()
        } else {
            None
        }
    }
}

fn build_menu(hmenu: HMENU, entries: &[EntryKind], id_map: &mut Vec<ItemId>) {
    for entry in entries {
        match entry {
            EntryKind::Text(e) => {
                id_map.push(e.id().clone());
                let id = id_map.len() as u32;
                let flags = MF_STRING | if !e.active() { MF_GRAYED } else { 0 };
                let text = encode_wide(e.text());
                unsafe { AppendMenuW(hmenu, flags, id as usize, text.as_ptr()) };
            }
            EntryKind::Check(e) => {
                id_map.push(e.id().clone());
                let id = id_map.len() as u32;
                let mut flags = MF_STRING;
                if e.ticked() {
                    flags |= MF_CHECKED;
                }
                if !e.active() {
                    flags |= MF_GRAYED;
                }
                let text = encode_wide(e.text());
                unsafe { AppendMenuW(hmenu, flags, id as usize, text.as_ptr()) };
            }
            EntryKind::Sub(sub) => {
                let submenu = unsafe { CreatePopupMenu() };
                build_menu(submenu, sub.entries(), id_map);
                let flags = MF_POPUP | if !sub.active() { MF_GRAYED } else { 0 };
                let text = encode_wide(sub.text());
                unsafe { AppendMenuW(hmenu, flags, submenu as usize, text.as_ptr()) };
            }
            EntryKind::Divider => {
                unsafe { AppendMenuW(hmenu, MF_SEPARATOR, 0, ptr::null()) };
            }
        }
    }
}

fn encode_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
