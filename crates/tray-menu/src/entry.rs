use std::borrow::Cow;

/// Unique identifier for a menu item.
///
/// Returned by [`PopupMenu::popup`] to indicate which item was selected.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ItemId(pub String);

impl ItemId {
    /// Creates a new item ID.
    pub fn of<S: Into<String>>(id: S) -> Self {
        Self(id.into())
    }
}

impl<S: Into<String>> From<S> for ItemId {
    fn from(s: S) -> Self {
        Self(s.into())
    }
}

impl AsRef<str> for ItemId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// The type of a menu entry.
#[derive(Clone)]
pub enum EntryKind {
    Text(TextEntry),
    Check(CheckEntry),
    Sub(SubMenu),
    Divider,
}

impl EntryKind {
    /// Returns the item ID if this is a selectable entry (text or check).
    pub fn item_id(&self) -> Option<&ItemId> {
        match self {
            EntryKind::Text(entry) => Some(&entry.id),
            EntryKind::Check(entry) => Some(&entry.id),
            EntryKind::Sub(_) => None,
            EntryKind::Divider => None,
        }
    }
}

/// A clickable text menu item.
#[derive(Clone)]
pub struct TextEntry {
    id: ItemId,
    text: Cow<'static, str>,
    active: bool,
}

impl TextEntry {
    /// Creates a new text entry with the given ID and label.
    pub fn of<S: Into<Cow<'static, str>>>(id: impl Into<ItemId>, text: S) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            active: true,
        }
    }

    /// Marks this entry as inactive (grayed out, non-clickable).
    pub fn inactive(mut self) -> Self {
        self.active = false;
        self
    }

    /// Returns the item ID.
    pub fn id(&self) -> &ItemId {
        &self.id
    }

    /// Returns the display text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns whether this entry is active (clickable).
    pub fn active(&self) -> bool {
        self.active
    }
}

/// A checkbox menu item.
#[derive(Clone)]
pub struct CheckEntry {
    id: ItemId,
    text: Cow<'static, str>,
    ticked: bool,
    active: bool,
}

impl CheckEntry {
    /// Creates a new checkbox entry with the given ID, label, and initial state.
    pub fn of<S: Into<Cow<'static, str>>>(id: impl Into<ItemId>, text: S, ticked: bool) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            ticked,
            active: true,
        }
    }

    /// Marks this entry as inactive (grayed out, non-clickable).
    pub fn inactive(mut self) -> Self {
        self.active = false;
        self
    }

    /// Returns the item ID.
    pub fn id(&self) -> &ItemId {
        &self.id
    }

    /// Returns the display text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns whether the checkbox is checked.
    pub fn ticked(&self) -> bool {
        self.ticked
    }

    /// Returns whether this entry is active (clickable).
    pub fn active(&self) -> bool {
        self.active
    }
}

/// A nested submenu.
#[derive(Clone)]
pub struct SubMenu {
    text: Cow<'static, str>,
    entries: Vec<EntryKind>,
    active: bool,
}

impl SubMenu {
    /// Creates a new submenu with the given label.
    pub fn of<S: Into<Cow<'static, str>>>(text: S) -> Self {
        Self {
            text: text.into(),
            entries: Vec::new(),
            active: true,
        }
    }

    /// Marks this submenu as inactive (grayed out, non-expandable).
    pub fn inactive(mut self) -> Self {
        self.active = false;
        self
    }

    /// Adds an entry to this submenu.
    pub fn add(&mut self, entry: &dyn AsEntry) {
        self.entries.push(entry.as_entry());
    }

    /// Returns the display text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the entries in this submenu.
    pub fn entries(&self) -> &[EntryKind] {
        &self.entries
    }

    /// Returns whether this submenu is active.
    pub fn active(&self) -> bool {
        self.active
    }
}

/// A visual separator line between menu items.
#[derive(Clone, Copy)]
pub struct Divider;

/// Trait for types that can be added to a menu.
///
/// Implemented by [`TextEntry`], [`CheckEntry`], [`SubMenu`], and [`Divider`].
pub trait AsEntry {
    /// Converts this type into an [`EntryKind`].
    fn as_entry(&self) -> EntryKind;
}

impl AsEntry for TextEntry {
    fn as_entry(&self) -> EntryKind {
        EntryKind::Text(self.clone())
    }
}

impl AsEntry for CheckEntry {
    fn as_entry(&self) -> EntryKind {
        EntryKind::Check(self.clone())
    }
}

impl AsEntry for SubMenu {
    fn as_entry(&self) -> EntryKind {
        EntryKind::Sub(self.clone())
    }
}

impl AsEntry for Divider {
    fn as_entry(&self) -> EntryKind {
        EntryKind::Divider
    }
}

/// A popup menu that can be displayed at a screen position.
///
/// Build a menu by adding entries with [`add`](Self::add), then display it
/// with [`popup`](Self::popup).
pub struct PopupMenu {
    entries: Vec<EntryKind>,
}

impl PopupMenu {
    /// Creates an empty popup menu.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Adds an entry to the menu.
    pub fn add(&mut self, entry: &dyn AsEntry) {
        self.entries.push(entry.as_entry());
    }

    /// Returns the entries in this menu.
    pub fn entries(&self) -> &[EntryKind] {
        &self.entries
    }
}

impl Default for PopupMenu {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_item_id_creation() {
        let id1 = ItemId::of("test");
        assert_eq!(id1.0, "test");

        let id2: ItemId = "test2".into();
        assert_eq!(id2.0, "test2");

        let id3: ItemId = String::from("test3").into();
        assert_eq!(id3.0, "test3");
    }

    #[test]
    fn test_item_id_equality() {
        let id1 = ItemId::of("same");
        let id2 = ItemId::of("same");
        let id3 = ItemId::of("different");

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_item_id_hash() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(ItemId::of("a"));
        set.insert(ItemId::of("b"));
        set.insert(ItemId::of("a"));

        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_text_entry_creation() {
        let entry = TextEntry::of("id1", "Label");
        assert_eq!(entry.id().0, "id1");
        assert_eq!(entry.text(), "Label");
        assert!(entry.active());
    }

    #[test]
    fn test_text_entry_inactive() {
        let entry = TextEntry::of("id1", "Label").inactive();
        assert!(!entry.active());
    }

    #[test]
    fn test_check_entry() {
        let entry = CheckEntry::of("check1", "Check Label", true);
        assert_eq!(entry.id().0, "check1");
        assert_eq!(entry.text(), "Check Label");
        assert!(entry.ticked());
        assert!(entry.active());

        let entry2 = CheckEntry::of("check2", "Unchecked", false).inactive();
        assert!(!entry2.ticked());
        assert!(!entry2.active());
    }

    #[test]
    fn test_submenu() {
        let mut submenu = SubMenu::of("File");
        assert_eq!(submenu.text(), "File");
        assert!(submenu.entries().is_empty());

        let entry = TextEntry::of("open", "Open");
        submenu.add(&entry);
        submenu.add(&Divider);

        assert_eq!(submenu.entries().len(), 2);
    }

    #[test]
    fn test_as_entry_trait() {
        let entry = TextEntry::of("id", "Label");
        let kind = entry.as_entry();
        assert!(matches!(kind, EntryKind::Text(_)));

        let check = CheckEntry::of("id", "Label", true);
        let kind = check.as_entry();
        assert!(matches!(kind, EntryKind::Check(_)));

        let submenu = SubMenu::of("Sub");
        let kind = submenu.as_entry();
        assert!(matches!(kind, EntryKind::Sub(_)));

        let divider = Divider;
        let kind = divider.as_entry();
        assert!(matches!(kind, EntryKind::Divider));
    }

    #[test]
    fn test_entry_kind_item_id() {
        let entry = TextEntry::of("item_id", "Label");
        let kind = entry.as_entry();
        assert_eq!(kind.item_id().map(|id| id.0.as_str()), Some("item_id"));

        let divider = Divider;
        let kind = divider.as_entry();
        assert!(kind.item_id().is_none());
    }
}
