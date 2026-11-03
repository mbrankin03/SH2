//! One-tap quick healing and inventory slots for lower screen touch UX.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickSlotItem {
    HealthDrink,
    FirstAidKit,
    Ampoule,
    HandgunBullets,
    ShotgunShells,
    RifleRounds,
}
