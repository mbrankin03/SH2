//! Inventory management, full 85-item Director's Cut catalog, item combinations (`86..=96`),
//! healing item execution, and 3D/2D item examination metadata (`SH2-15`).
//!
//! Grounded in `sh2pc.exe` (Director's Cut) item tables (`0x892640`, `0x88d4f8`, `0x891858`,
//! `0x49e310`, `0x49e4e0`, `0x49e7e0`) and `data/etc/message/item_x_msg_e.mes`.

mod catalog;
pub use catalog::*;

/// Maximum quantity of any stackable item in the player's inventory (`0x49e310`).
pub const MAX_ITEM_STACK: u16 = 999;

/// Total number of standard inventory items in the Director's Cut catalog (`1..=85`).
pub const CATALOG_ITEM_COUNT: usize = 85;

/// First combination pseudo-item ID (`"Purple Bull" key + "Lapis Eye" key`).
pub const FIRST_COMBO_ITEM_ID: u8 = 86;

/// Last combination pseudo-item ID (`Horseshoe + Lighter + Wax doll`).
pub const LAST_COMBO_ITEM_ID: u8 = 96;

/// Ampoule stamina buff duration in seconds (`0x1f7d40c` in `0x49e7e0`).
pub const AMPOULE_STAMINA_DURATION_SECS: f32 = 600.0;

/// High-level functional classification for Silent Hill 2 inventory items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ItemCategory {
    /// Restorative medical consumables (Health drink, First-aid kit, Ampoule).
    Consumable,
    /// Equippable ranged or melee weapons.
    Weapon,
    /// Stackable firearm ammunition affected by the `Bullet Adjust` multiplier (`1..=3`).
    Ammunition,
    /// Utility equipment (Flashlight, Radio).
    Equipment,
    /// Doors, gates, and progression keys.
    KeyItem,
    /// Combinable or inspectable puzzle objects (Coins, Tablets, Music Boxes, Rings, etc.).
    PuzzleItem,
    /// New Game+ / Rebirth / Dog / UFO secret items tracked in `gSecretItemsCollected`.
    SecretItem,
    /// *Born from a Wish* (`Maria` sub-scenario) exclusive items (`79..=85`).
    SubScenarioItem,
}

/// Static metadata for a single item in the Director's Cut catalog (`1..=85`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemMetadata {
    /// 1-based item identifier (`1..=85`).
    pub id: u8,
    /// Character/model kind ID (`0x0700..`, or `0` if 2D-only).
    pub model_kind: u16,
    /// Display name from `data/etc/message/item_x_msg_e.mes` (`18 + 2 * id`).
    pub name: &'static str,
    /// Primary inventory description from `data/etc/message/item_x_msg_e.mes` (`19 + 2 * id`).
    pub description: &'static str,
    /// Extended examination text when inspected closely (`Examine` action), if any.
    pub examine_text: Option<&'static str>,
    /// Functional item category.
    pub category: ItemCategory,
    /// Relative 3D `.mdl` asset path for 3D rotating item inspection, if available.
    pub model_path: Option<&'static str>,
    /// Relative 2D `.tex` cutaway still path for 2D close-up examination, if available.
    pub inspect_texture_path: Option<&'static str>,
    /// Base pickup quantity before `Bullet Adjust` multiplier.
    pub base_pickup_amount: u16,
}

impl ItemMetadata {
    /// Returns `true` if this item stacks numerically in the inventory (`1..=12` except weapons).
    pub const fn is_stackable(&self) -> bool {
        matches!(self.id, 1 | 2 | 3 | 5 | 7 | 9 | 11)
    }

    /// Returns `true` if this item is ammunition affected by the `Bullet Adjust` option (`1..=3`).
    pub const fn is_ammo(&self) -> bool {
        matches!(self.id, 5 | 7 | 9 | 11)
    }
}

/// Recipe mapping 2 or 3 sorted inventory item IDs to a combination pseudo-item (`86..=96`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemCombination {
    /// Resulting pseudo-item ID (`86..=96`).
    pub result_id: u8,
    /// Sorted ingredient item IDs (`0` in `parts[0]` indicates a 2-item combination).
    pub parts: [u8; 3],
    /// Whether `ItemLose` consumes the constituent items when this combination is used.
    /// Note: Combinations `93..=95` (incomplete pairs of Horseshoe/Lighter/Wax Doll) do not
    /// consume their parts; only the complete 3-item combination `96` consumes `62, 63, 64`.
    pub consumes_parts_on_use: bool,
}

/// Result of consuming a restorative healing item (`0x49e7e0`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HealResult {
    /// New player HP after clamping to `max_hp`.
    pub new_hp: f32,
    /// Fraction of `max_hp` that overflowed `max_hp` (accumulated into `0x1f7d9d4` for ending scoring).
    pub overheal_fraction: f32,
    /// Whether the Ampoule 600-second anti-fatigue stamina timer was activated.
    pub activated_ampoule_stamina: bool,
}

/// Inspection payload returned when examining an item in the inventory screen (`0x49f710`).
#[derive(Debug, Clone, PartialEq)]
pub struct ItemExamineView {
    /// Examined item ID (`1..=85`).
    pub item_id: u8,
    /// Item display name.
    pub name: &'static str,
    /// Primary description text.
    pub description: &'static str,
    /// Secondary close-up examination text (if any).
    pub examine_text: Option<&'static str>,
    /// Relative 3D `.mdl` path for rotating 3D item preview.
    pub model_path: Option<&'static str>,
    /// Relative 2D `.tex` path for 2D cutaway examination overlay.
    pub inspect_texture_path: Option<&'static str>,
    /// Current 3D preview rotation angle in radians.
    pub rotation_angle_rad: f32,
}

/// Runtime player inventory, item combination, and examination state machine.
#[derive(Debug, Clone)]
pub struct InventoryManager {
    /// Quantity owned per item ID (`0..256`).
    counts: [u16; 256],
    /// `Bullet Adjust` option multiplier (`1..=3`, PC `0x1dbec05`).
    bullet_adjust: u8,
    /// Bitmask of the 8 secret items collected (`gSecretItemsCollected`).
    secret_items_mask: u8,
    /// Total standard items picked up (excludes secret items and Mary's photo `20` / letter `21`).
    total_items_picked_up: u16,
    /// Number of times Mary's photo (`20`) or letter (`21`) has been examined (capped at `2`, `0x1f7da7a`).
    mary_examine_count: u8,
    /// Accumulated overheal fraction from healing items (`0x1f7d9d4`).
    overheal_score: f32,
    /// Remaining duration in seconds for the Ampoule stamina buff (`0x1f7d40c`).
    ampoule_stamina_timer: f32,
    /// Current 3D item viewer rotation angle in radians.
    viewer_rotation_rad: f32,
}

impl Default for InventoryManager {
    fn default() -> Self {
        Self::new()
    }
}

impl InventoryManager {
    /// Creates an empty inventory with `Bullet Adjust` set to `1x`.
    pub fn new() -> Self {
        Self {
            counts: [0; 256],
            bullet_adjust: 1,
            secret_items_mask: 0,
            total_items_picked_up: 0,
            mary_examine_count: 0,
            overheal_score: 0.0,
            ampoule_stamina_timer: 0.0,
            viewer_rotation_rad: 0.0,
        }
    }

    /// Resets all inventory counts and counters for a new game.
    pub fn reset(&mut self) {
        self.counts = [0; 256];
        self.secret_items_mask = 0;
        self.total_items_picked_up = 0;
        self.mary_examine_count = 0;
        self.overheal_score = 0.0;
        self.ampoule_stamina_timer = 0.0;
        self.viewer_rotation_rad = 0.0;
    }

    /// Returns the active `Bullet Adjust` multiplier (`1..=3`).
    pub fn bullet_adjust(&self) -> u8 {
        self.bullet_adjust
    }

    /// Sets the `Bullet Adjust` multiplier (clamped to `1..=3`).
    pub fn set_bullet_adjust(&mut self, factor: u8) {
        self.bullet_adjust = factor.clamp(1, 3);
    }

    /// Returns the quantity of `item_id` currently in the inventory.
    pub fn count(&self, item_id: u8) -> u16 {
        self.counts[item_id as usize]
    }

    /// Returns `true` if the player owns at least one of `item_id`, or all parts of a combination
    /// pseudo-item (`86..=96`).
    pub fn has_item(&self, item_id: u8) -> bool {
        if (FIRST_COMBO_ITEM_ID..=LAST_COMBO_ITEM_ID).contains(&item_id) {
            return self.owns_combo_parts(item_id);
        }
        self.counts[item_id as usize] > 0
    }

    /// Directly adjusts the count of `item_id` by `delta`, clamping to `0..=999`.
    pub fn give_raw(&mut self, item_id: u8, delta: i32) {
        if item_id == 0 {
            return;
        }
        let cur = self.counts[item_id as usize] as i32;
        self.counts[item_id as usize] = (cur + delta).clamp(0, MAX_ITEM_STACK as i32) as u16;
    }

    /// Computes the pickup quantity for `item_id` (`0x49e310`), applying the `Bullet Adjust`
    /// multiplier (`1..=3`) to ammunition items (`5, 7, 9, 11`).
    pub fn pickup_amount_for(&self, item_id: u8) -> u16 {
        let Some(meta) = item_metadata(item_id) else {
            return 1;
        };
        if meta.is_ammo() {
            meta.base_pickup_amount
                .saturating_mul(self.bullet_adjust as u16)
        } else {
            meta.base_pickup_amount
        }
    }

    /// Executes `ItemGet` (`0x49e310`): records secret item bitmask or total pickup statistics,
    /// and adds the item (or stackable box quantity capped at `999`) to the inventory.
    pub fn item_get_now(&mut self, item_id: u8) -> u16 {
        if item_id == 0 {
            return 0;
        }
        self.record_pickup_stat(item_id);

        let is_counted = (1..=12).contains(&item_id);
        if !is_counted {
            if self.counts[item_id as usize] == 0 {
                self.counts[item_id as usize] = 1;
            }
            return self.counts[item_id as usize];
        }

        let add = self.pickup_amount_for(item_id);
        let cur = self.counts[item_id as usize];
        let next = cur.saturating_add(add).min(MAX_ITEM_STACK);
        self.counts[item_id as usize] = next;
        next
    }

    /// Executes `ItemLose` (`0x49e4e0`): removes 1 unit from stackable consumables/ammo, removes
    /// single-instance key/puzzle items, or consumes combination ingredients (`86..=96`).
    pub fn item_lose(&mut self, item_id: u8) {
        if (FIRST_COMBO_ITEM_ID..=LAST_COMBO_ITEM_ID).contains(&item_id) {
            if let Some(combo) = ITEM_COMBOS.iter().find(|c| c.result_id == item_id) {
                if combo.consumes_parts_on_use {
                    for &part in &combo.parts {
                        if part != 0 {
                            self.counts[part as usize] = 0;
                        }
                    }
                }
            }
            return;
        }

        let cur = self.counts[item_id as usize];
        if cur == 0 {
            return;
        }
        let is_stackable = matches!(item_id, 1 | 2 | 3 | 5 | 7 | 9 | 11);
        if is_stackable {
            self.counts[item_id as usize] = cur - 1;
        } else {
            self.counts[item_id as usize] = 0;
        }
    }

    /// Returns `true` if the player owns every constituent part of combination `combo_id` (`86..=96`).
    pub fn owns_combo_parts(&self, combo_id: u8) -> bool {
        let Some(combo) = ITEM_COMBOS.iter().find(|c| c.result_id == combo_id) else {
            return false;
        };
        combo
            .parts
            .iter()
            .all(|&part| part == 0 || self.counts[part as usize] > 0)
    }

    /// Attempts to combine 2 or 3 owned items into a combination pseudo-item (`86..=96`).
    pub fn try_combine_owned(&self, a: u8, b: u8, c: u8) -> Option<u8> {
        for &id in &[a, b, c] {
            if id != 0 && self.counts[id as usize] == 0 {
                return None;
            }
        }
        combine_items(a, b, c)
    }

    /// Uses a restorative healing item (`1` = Health Drink +25%, `2` = First-Aid Kit +50%,
    /// `3` = Ampoule +100% + 600s stamina timer), consuming 1 unit and tracking overheal fraction.
    pub fn use_healing_item(
        &mut self,
        item_id: u8,
        current_hp: f32,
        max_hp: f32,
    ) -> Option<HealResult> {
        if !(1..=3).contains(&item_id) || self.counts[item_id as usize] == 0 {
            return None;
        }
        let max_hp = max_hp.max(1.0);
        let fraction = match item_id {
            1 => 0.25,
            2 => 0.50,
            3 => 1.00,
            _ => unreachable!(),
        };
        self.item_lose(item_id);

        let heal_amount = max_hp * fraction;
        let unclamped = current_hp + heal_amount;
        let overheal_fraction = if unclamped > max_hp {
            (unclamped - max_hp) / max_hp
        } else {
            0.0
        };
        self.overheal_score += overheal_fraction;

        let activated_ampoule_stamina = if item_id == 3 {
            self.ampoule_stamina_timer = AMPOULE_STAMINA_DURATION_SECS;
            true
        } else {
            false
        };

        Some(HealResult {
            new_hp: unclamped.min(max_hp),
            overheal_fraction,
            activated_ampoule_stamina,
        })
    }

    /// Advances the 3D item viewer rotation and Ampoule stamina timer by `dt` seconds.
    pub fn tick(&mut self, dt: f32) {
        if dt > 0.0 {
            self.ampoule_stamina_timer = (self.ampoule_stamina_timer - dt).max(0.0);
            self.viewer_rotation_rad =
                (self.viewer_rotation_rad + dt * 1.2) % std::f32::consts::TAU;
        }
    }

    /// Examines an item (`0x471590` / `0x49f710`), updating ending score counters (Mary's photo/letter
    /// examination count up to `2`, and returning `true` in `sets_angela_knife_flag_630` when
    /// Angela's knife `24` is examined).
    pub fn examine_item(&mut self, item_id: u8) -> Option<(ItemExamineView, bool)> {
        let meta = item_metadata(item_id)?;
        if (item_id == 20 || item_id == 21) && self.mary_examine_count < 2 {
            self.mary_examine_count += 1;
        }
        let sets_flag_630 = item_id == 24;
        Some((
            ItemExamineView {
                item_id,
                name: meta.name,
                description: meta.description,
                examine_text: meta.examine_text,
                model_path: meta.model_path,
                inspect_texture_path: meta.inspect_texture_path,
                rotation_angle_rad: self.viewer_rotation_rad,
            },
            sets_flag_630,
        ))
    }

    /// Returns the sorted list of owned item IDs (`1..=85`) for UI display.
    pub fn owned_items(&self) -> Vec<(u8, u16)> {
        (1..=85u8)
            .filter_map(|id| {
                let c = self.counts[id as usize];
                if c > 0 {
                    Some((id, c))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Bitmask of secret items collected (`gSecretItemsCollected`).
    pub fn secret_items_mask(&self) -> u8 {
        self.secret_items_mask
    }

    /// Number of distinct secret items collected (`0..=8`).
    pub fn secret_items_count(&self) -> u8 {
        self.secret_items_mask.count_ones() as u8
    }

    /// Total non-secret items picked up (excluding Mary's photo `20` and letter `21`).
    pub fn total_items_picked_up(&self) -> u16 {
        self.total_items_picked_up
    }

    /// Number of times Mary's photo (`20`) or letter (`21`) was examined (`0..=2`).
    pub fn mary_examine_count(&self) -> u8 {
        self.mary_examine_count
    }

    /// Accumulated overheal fraction for ending determination.
    pub fn overheal_score(&self) -> f32 {
        self.overheal_score
    }

    /// Remaining seconds on the Ampoule stamina buff (`0x1f7d40c`).
    pub fn ampoule_stamina_timer(&self) -> f32 {
        self.ampoule_stamina_timer
    }

    fn record_pickup_stat(&mut self, item_id: u8) {
        const SECRET_BITS: [(u8, u8); 8] = [
            (0x10, 0x01), // Chainsaw (16)
            (0x0c, 0x02), // Hyper spray (12)
            (0x19, 0x04), // Dog key (25)
            (0x4a, 0x08), // Book: "Lost Memories" (74)
            (0x4b, 0x10), // Book: "Crimson Ceremony" (75)
            (0x4c, 0x20), // White chrism (76)
            (0x4d, 0x40), // Obsidian goblet (77)
            (0x4e, 0x80), // Blue Gem (78)
        ];
        for &(id, bit) in &SECRET_BITS {
            if item_id == id {
                self.secret_items_mask |= bit;
                return;
            }
        }
        if item_id != 20 && item_id != 21 {
            self.total_items_picked_up = self.total_items_picked_up.saturating_add(1);
        }
    }
}
