//! One-tap quick healing and inventory slots for lower screen touch UX.
//!
//! # Companion Inventory Architecture
//! Allows instantaneous item usage (health recovery drinks, ammunition reload) directly
//! from the companion touchscreen without pausing into the full-screen inventory menu.

/// Consumable and ammunition items assignable to bottom screen quick touch slots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickSlotItem {
    /// Restores 25% health.
    HealthDrink,
    /// Restores 50% health.
    FirstAidKit,
    /// Fully restores health and resets stamina.
    Ampoule,
    /// Handgun 9mm magazine reload.
    HandgunBullets,
    /// 12 Gauge shotgun shells reload.
    ShotgunShells,
    /// Hunting rifle ammunition reload.
    RifleRounds,
}
