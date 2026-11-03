//! sh2-ui: Dual-screen companion UI, dynamic map (Maps 1-36), quick-slots, touch dev console.

pub mod map;
pub mod quickslots;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActivePanel {
    #[default]
    Map,
    Memos,
    QuickSlots,
    DevConsole,
}
