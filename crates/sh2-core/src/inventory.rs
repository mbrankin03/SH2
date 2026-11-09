//! Inventory management, full 85-item Director's Cut catalog, item combinations (`86..=96`),
//! healing item execution, and 3D/2D item examination metadata (`SH2-15`).
//!
//! Grounded in `sh2pc.exe` (Director's Cut) item tables (`0x892640`, `0x88d4f8`, `0x891858`,
//! `0x49e310`, `0x49e4e0`, `0x49e7e0`) and `data/etc/message/item_x_msg_e.mes`.

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

/// Complete table of the 11 item combinations at `0x891858` in `sh2pc.exe`.
pub const ITEM_COMBOS: [ItemCombination; 11] = [
    // 86 (0x56): "Purple Bull" key (35) + "Lapis Eye" key (36)
    ItemCombination {
        result_id: 86,
        parts: [0, 35, 36],
        consumes_parts_on_use: true,
    },
    // 87 (0x57): Piece of hair (53) + Bent needle (54)
    ItemCombination {
        result_id: 87,
        parts: [0, 53, 54],
        consumes_parts_on_use: true,
    },
    // 88 (0x58): Copper ring (56) + Lead ring (57)
    ItemCombination {
        result_id: 88,
        parts: [0, 56, 57],
        consumes_parts_on_use: true,
    },
    // 89 (0x59): Tablet of "The Oppressor" (59) + Tablet of "Gluttonous Pig" (60)
    ItemCombination {
        result_id: 89,
        parts: [0, 59, 60],
        consumes_parts_on_use: true,
    },
    // 90 (0x5a): Tablet of "The Oppressor" (59) + Tablet of "The Seductress" (61)
    ItemCombination {
        result_id: 90,
        parts: [0, 59, 61],
        consumes_parts_on_use: true,
    },
    // 91 (0x5b): Tablet of "Gluttonous Pig" (60) + Tablet of "The Seductress" (61)
    ItemCombination {
        result_id: 91,
        parts: [0, 60, 61],
        consumes_parts_on_use: true,
    },
    // 92 (0x5c): All three Prison Tablets (59 + 60 + 61)
    ItemCombination {
        result_id: 92,
        parts: [59, 60, 61],
        consumes_parts_on_use: true,
    },
    // 93 (0x5d): Horseshoe (62) + Lighter (63) (incomplete, does not consume parts)
    ItemCombination {
        result_id: 93,
        parts: [0, 62, 63],
        consumes_parts_on_use: false,
    },
    // 94 (0x5e): Horseshoe (62) + Wax doll (64) (incomplete, does not consume parts)
    ItemCombination {
        result_id: 94,
        parts: [0, 62, 64],
        consumes_parts_on_use: false,
    },
    // 95 (0x5f): Lighter (63) + Wax doll (64) (incomplete, does not consume parts)
    ItemCombination {
        result_id: 95,
        parts: [0, 63, 64],
        consumes_parts_on_use: false,
    },
    // 96 (0x60): Horseshoe (62) + Lighter (63) + Wax doll (64)
    ItemCombination {
        result_id: 96,
        parts: [62, 63, 64],
        consumes_parts_on_use: true,
    },
];

/// System UI strings from `data/etc/message/item_x_msg_e.mes` (`0..=19`).
pub const ITEM_SYSTEM_MESSAGES: [&str; 20] = [
    "Use",
    "Reload",
    "Adjust volume",
    "Power switch",
    "On",
    "Off",
    "Examine",
    "Equip",
    "Unequip",
    "Option",
    "Map",
    "Memo",
    "Status",
    "Equipment",
    "Command",
    "Combine",
    "It looks like I can't use that here.",
    "There's no use for just this.",
    "This combination doesn't make\nany sense.",
    "Put on shelf",
];

/// Full 85-item Director's Cut catalog (`1..=85`).
pub const ITEM_CATALOG: [ItemMetadata; CATALOG_ITEM_COUNT] = [
    ItemMetadata {
        id: 1,
        model_kind: 0x0700,
        name: "Health drink(s)",
        description: "Restores energy by providing nourishment.\nEffects are fairly mild.",
        examine_text: None,
        category: ItemCategory::Consumable,
        model_path: Some("data/chr/item/x_drink.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 2,
        model_kind: 0x0701,
        name: "First-aid kit(s)",
        description: "Restores energy by healing wounds.\nEffects are moderate.",
        examine_text: None,
        category: ItemCategory::Consumable,
        model_path: Some("data/chr/item/x_firstaid.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 3,
        model_kind: 0x0733,
        name: "Ampoule(s)",
        description: "Restores energy by relieving pain.\nEffects are powerful.",
        examine_text: None,
        category: ItemCategory::Consumable,
        model_path: Some("data/chr/item/x_ample.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 4,
        model_kind: 0x0702,
        name: "Handgun",
        description: "Full magazine holds 10 bullets. Light handgun.\nNot much stopping power, but easy to use.",
        examine_text: None,
        category: ItemCategory::Weapon,
        model_path: Some("data/chr/item/x_handgun.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 5,
        model_kind: 0x0703,
        name: "Handgun bullet(s)",
        description: "Bullets for a handgun.",
        examine_text: None,
        category: ItemCategory::Ammunition,
        model_path: Some("data/chr/item/x_handbul.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 10,
    },
    ItemMetadata {
        id: 6,
        model_kind: 0x0743,
        name: "Shotgun",
        description: "Maximum capacity 6 shots. Tough to use,\nbut can attack opponents in a group.",
        examine_text: None,
        category: ItemCategory::Weapon,
        model_path: Some("data/chr/item/x_wp_shotgun.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 7,
        model_kind: 0x0724,
        name: "Shotgun shell(s)",
        description: "Ammunition for a shotgun.",
        examine_text: None,
        category: ItemCategory::Ammunition,
        model_path: Some("data/chr/item/x_shotbul.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 6,
    },
    ItemMetadata {
        id: 8,
        model_kind: 0x073b,
        name: "Hunting rifle",
        description: "Maximum capacity 4 shots. Takes a long time\nbetween shots, but each shot is very powerful.",
        examine_text: None,
        category: ItemCategory::Weapon,
        model_path: Some("data/chr/item/x_wp_riflgun.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 9,
        model_kind: 0x0723,
        name: "Rifle shell(s)",
        description: "Ammunition for a hunting rifle.",
        examine_text: None,
        category: ItemCategory::Ammunition,
        model_path: Some("data/chr/item/x_riflebul.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 4,
    },
    ItemMetadata {
        id: 10,
        model_kind: 0x0751,
        name: "Revolver",
        description: "Full chamber holds 10 bullets.\nEasy to use, but not much power.",
        examine_text: None,
        category: ItemCategory::Weapon,
        model_path: None,
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 11,
        model_kind: 0x0749,
        name: "Revolver bullet(s)",
        description: "Bullets for a revolver.",
        examine_text: None,
        category: ItemCategory::Ammunition,
        model_path: Some("data/chr2/item/x_bullt_revolver.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 6,
    },
    ItemMetadata {
        id: 12,
        model_kind: 0x0744,
        name: "Hyper spray",
        description: "Suspicious spray can.\nIts effects are unknown.",
        examine_text: None,
        category: ItemCategory::SecretItem,
        model_path: Some("data/chr/item/x_wp_sp.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 13,
        model_kind: 0x0704,
        name: "Wooden plank",
        description: "Wooden plank with nails on one end.\nNot very powerful, but easy to use.",
        examine_text: None,
        category: ItemCategory::Weapon,
        model_path: None,
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 14,
        model_kind: 0x0745,
        name: "Steel pipe",
        description: "3-foot long steel pipe.\nNot very destructive, but good range.",
        examine_text: None,
        category: ItemCategory::Weapon,
        model_path: Some("data/chr/item/x_wp_pipe.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 15,
        model_kind: 0x050c,
        name: "Great knife",
        description: "Massive weapon wielded by Pyramid Head.\nHard to use, but lethal.",
        examine_text: None,
        category: ItemCategory::Weapon,
        model_path: Some("data/chr/item/nat.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 16,
        model_kind: 0x0746,
        name: "Chainsaw",
        description: "Gas-powered chainsaw.\nExtremely high attack power.",
        examine_text: None,
        category: ItemCategory::SecretItem,
        model_path: Some("data/chr/item/x_wp_csaw.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 17,
        model_kind: 0x074a,
        name: "Chinese cleaver",
        description: "Square-bladed knife.\nUgly, but powerful and easy to use.",
        examine_text: None,
        category: ItemCategory::Weapon,
        model_path: Some("data/chr2/item/x_chinanife.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 18,
        model_kind: 0x0705,
        name: "Flashlight",
        description: "One-touch ON/OFF power switch.\nCurrently in breast pocket.",
        examine_text: Some("Is this thing out of batteries?\nIt won't light up when I press the switch."),
        category: ItemCategory::Equipment,
        model_path: Some("data/chr/item/x_jlight.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 19,
        model_kind: 0x0706,
        name: "Radio",
        description: "Small, portable radio.\nEmits static when monsters are nearby.",
        examine_text: Some("Is this thing broken?\nPressing the switch doesn't do anything."),
        category: ItemCategory::Equipment,
        model_path: None,
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 20,
        model_kind: 0x070c,
        name: "Photo of Mary",
        description: "A photo from when she was still healthy.",
        examine_text: None,
        category: ItemCategory::KeyItem,
        model_path: None,
        inspect_texture_path: Some("pic/item/x_mary_p.tex"),
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 21,
        model_kind: 0x070b,
        name: "Letter from Mary",
        description: "That's definitely Mary's name in her own\nhandwriting on the front of the envelope.",
        examine_text: Some("In my restless dreams, \nI see that town.\nSilent Hill.\x0cYou promised you'd take me\nthere again someday.\nBut you never did.\x0cWell, I'm alone there now...\nIn our \"special place...\"\x0cWaiting for you..."),
        category: ItemCategory::KeyItem,
        model_path: None,
        inspect_texture_path: Some("pic/item/p_letterm.tex"),
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 22,
        model_kind: 0x0000,
        name: "Laura's letter",
        description: "Letter supposedly given to Laura by Mary.",
        examine_text: Some("My dearest Laura, \nI'm leaving this letter with Rachel\nto give to you after I'm gone.\x0cI'm far away now. \nIn a quiet, beautiful place.\x0cPlease forgive me for not saying\ngoodbye before I left.\x0cBe well, Laura.\x0cDon't be too hard on the sisters.\x0cAnd Laura, about James...\nI know you hate him because you\nthink he isn't nice to me, but please\ngive him a chance.\x0cIt's true he may be a little surly\nsometimes, and he doesn't laugh\nmuch. But underneath he's really\na sweet person.\x0cLaura...\nI love you like my very own\ndaughter.\x0cIf things had worked out\ndifferently, I was hoping to\nadopt you.\x0cHappy 8th birthday, Laura.\x0cYour friend forever,\nMary"),
        category: ItemCategory::KeyItem,
        model_path: None,
        inspect_texture_path: Some("pic/item/p_laura_letter.tex"),
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 23,
        model_kind: 0x070e,
        name: "Videotape",
        description: "Videotape I forgot at the hotel three years ago.",
        examine_text: Some("\"Silent Hill\" is written on the label\nin my handwriting."),
        category: ItemCategory::KeyItem,
        model_path: Some("data/chr/item/x_video.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 24,
        model_kind: 0x0000,
        name: "Angela's knife",
        description: "Knife from Angela. I don't plan on\nusing this as a weapon.",
        examine_text: Some("The blade is stained with something red."),
        category: ItemCategory::KeyItem,
        model_path: None,
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 25,
        model_kind: 0x0000,
        name: "Dog key",
        description: "Found inside a dog house.\nNo idea where to use it.",
        examine_text: None,
        category: ItemCategory::SecretItem,
        model_path: None,
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 26,
        model_kind: 0x0722,
        name: "Apartment gate key",
        description: "Old-looking key that I found in town.",
        examine_text: None,
        category: ItemCategory::KeyItem,
        model_path: Some("data/chr/item/x_keygate.mdl"),
        inspect_texture_path: Some("pic/item/x_keygate.tex"),
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 27,
        model_kind: 0x072f,
        name: "Key to room 202",
        description: "\"202\" is written on the top of the key.\nGot it in room 208.",
        examine_text: None,
        category: ItemCategory::KeyItem,
        model_path: Some("data/chr/item/x_key202.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 28,
        model_kind: 0x041f,
        name: "Clock key",
        description: "There's a small clock at the end of the key.\nIt was in a hole in the wall in room 202.",
        examine_text: Some("Now that I think of it, there was a\nbig clock in room 208..."),
        category: ItemCategory::KeyItem,
        model_path: None,
        inspect_texture_path: Some("pic/item/x_keyclock.tex"),
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 29,
        model_kind: 0x072c,
        name: "Courtyard key",
        description: "Key to door which leads from the westside\nstairway hall to the courtyard.",
        examine_text: Some("It says \"courtyard.\""),
        category: ItemCategory::KeyItem,
        model_path: Some("data/chr/item/x_keyemerg.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 30,
        model_kind: 0x0732,
        name: "Fire escape key",
        description: "Key to door leading from hallway to fire\nescape. I found it on the 3rd floor hall.",
        examine_text: Some("The tag attached to it says\n\"fire escape.\""),
        category: ItemCategory::KeyItem,
        model_path: Some("data/chr/item/x_keycourt.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 31,
        model_kind: 0x0731,
        name: "Lyne house key",
        description: "It was in the coin puzzle desk.\nThe tag on it says \"Lyne.\"",
        examine_text: Some("This must be the key mentioned in that note.\nThe one that says it was \"left with Uncle David.\""),
        category: ItemCategory::KeyItem,
        model_path: Some("data/chr/item/x_keylyne.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 32,
        model_kind: 0x0730,
        name: "Apartment stairway key",
        description: "Key to door that leads from the westside\napartment hall to the stairway.",
        examine_text: Some("It says \"Northside Stairway.\""),
        category: ItemCategory::KeyItem,
        model_path: Some("data/chr/item/x_keynorth.mdl"),
        inspect_texture_path: Some("pic/item/x_keynorth.tex"),
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 33,
        model_kind: 0x0000,
        name: "Examination room key",
        description: "\"Examination room\" is written on the tag.\nFound it in the pocket of the white lab jacket\nin the men's locker room.",
        examine_text: None,
        category: ItemCategory::KeyItem,
        model_path: None,
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 34,
        model_kind: 0x0729,
        name: "Roof key",
        description: "Key to roof of office wing. Got it in the\n3rd floor hospital room where Maria is resting.",
        examine_text: None,
        category: ItemCategory::KeyItem,
        model_path: Some("data/chr/item/x_keyroof.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 35,
        model_kind: 0x0707,
        name: "\"Purple Bull\" key",
        description: "A purple bull is drawn on it.\nIt was on the desk in the document room.",
        examine_text: None,
        category: ItemCategory::KeyItem,
        model_path: Some("data/chr/item/x_keypurple.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 36,
        model_kind: 0x0728,
        name: "\"Lapis Eye\" key",
        description: "An eye is carved into the top of the key.\nThe iris part is made of a lapis lazuli.",
        examine_text: Some("A snake is carved into its stem."),
        category: ItemCategory::KeyItem,
        model_path: Some("data/chr/item/x_keyrapis.mdl"),
        inspect_texture_path: Some("pic/item/x_keyrapis.tex"),
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 37,
        model_kind: 0x070a,
        name: "Elevator key",
        description: "\"Patient wing elevator\" is written on the tag.\nIt was stuck in the bath pipe.",
        examine_text: None,
        category: ItemCategory::KeyItem,
        model_path: Some("data/chr/item/x_keyelevator.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 38,
        model_kind: 0x072a,
        name: "Basement storeroom key",
        description: "It was inside the wall with that strange\npainting. It's the key to the hospital\nbasement's storeroom.",
        examine_text: None,
        category: ItemCategory::KeyItem,
        model_path: Some("data/chr/item/x_keybase.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 39,
        model_kind: 0x071b,
        name: "Hospital lobby key",
        description: "Found in the Director's office.\nKey to hospital's front lobby.",
        examine_text: None,
        category: ItemCategory::KeyItem,
        model_path: Some("data/chr/item/x_keyhos.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 40,
        model_kind: 0x0726,
        name: "Old bronze key",
        description: "Antique-looking bronze key. It was hidden\nbehind one of those statues lined up in the park.",
        examine_text: None,
        category: ItemCategory::KeyItem,
        model_path: None,
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 41,
        model_kind: 0x073f,
        name: "Spiral-writing key",
        description: "Key with a slim, 3-inch long cylinder attached.\nThe cylinder is engraved with spiral writing.",
        examine_text: Some("It says \"'Tis doubt which leadeth thee\nto Purgatory.\""),
        category: ItemCategory::KeyItem,
        model_path: Some("data/chr/item/x_keyspiral.mdl"),
        inspect_texture_path: Some("pic/item/x_keyspiral.tex"),
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 42,
        model_kind: 0x0742,
        name: "Key of the Persecuted",
        description: "Key left by prisoner who was wrongly\nexecuted.",
        examine_text: None,
        category: ItemCategory::KeyItem,
        model_path: Some("data/chr/item/x_keyfalse.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 43,
        model_kind: 0x073d,
        name: "Key to hotel room 312",
        description: "Key to room number 312 in the hotel.\nIt was lying behind the counter of the lobby\nreception desk.",
        examine_text: Some("Room 312 is the room Mary and\nI stayed in three years ago."),
        category: ItemCategory::KeyItem,
        model_path: Some("data/chr/item/x_key312.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 44,
        model_kind: 0x0000,
        name: "Key to hotel room 204",
        description: "Key to room number 204.\nTook it from the suitcase inside the cloakroom.",
        examine_text: None,
        category: ItemCategory::KeyItem,
        model_path: None,
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 45,
        model_kind: 0x073e,
        name: "Employee elevator key",
        description: "The tag on it says \"Employee elevator.\"\nFound it in room 204.",
        examine_text: None,
        category: ItemCategory::KeyItem,
        model_path: Some("data/chr/item/x_keyemploy.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 46,
        model_kind: 0x0735,
        name: "Bar key",
        description: "Key to door which goes from hotel's underground\nbar to hallway. Found it in the boiler room.",
        examine_text: None,
        category: ItemCategory::KeyItem,
        model_path: Some("data/chr/item/x_keybar.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 47,
        model_kind: 0x0736,
        name: "\"Fish\" key",
        description: "Key with a fish-shaped keychain.\nIt was on top of the restaurant table.",
        examine_text: None,
        category: ItemCategory::KeyItem,
        model_path: Some("data/chr/item/x_keyfish.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 48,
        model_kind: 0x0747,
        name: "Hotel stairway key",
        description: "Key to door from stairway to 3rd floor hall.\nIt was hidden inside the big music box in the lobby.",
        examine_text: None,
        category: ItemCategory::KeyItem,
        model_path: Some("data/chr/item/x_key3f.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 49,
        model_kind: 0x072d,
        name: "Canned juice",
        description: "A heavy six-pack of expired canned juice.\nIt was in the hallway on the 1st floor.",
        examine_text: None,
        category: ItemCategory::PuzzleItem,
        model_path: Some("data/chr/item/x_juice.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 50,
        model_kind: 0x071d,
        name: "Coin [Snake]",
        description: "Large coin imprinted with a snake.\nI found it in that pool with no water.",
        examine_text: Some("The reverse side of the coin is a crescent moon."),
        category: ItemCategory::PuzzleItem,
        model_path: Some("data/chr/item/x_coinsnake.mdl"),
        inspect_texture_path: Some("pic/item/x_coinsnake.tex"),
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 51,
        model_kind: 0x071f,
        name: "Coin [Old Man]",
        description: "Large coin imprinted with the face of an\nold person. Took it out of the trash chute exit.",
        examine_text: Some("The reverse side shows a gravestone."),
        category: ItemCategory::PuzzleItem,
        model_path: Some("data/chr/item/x_coinelder.mdl"),
        inspect_texture_path: Some("pic/item/x_coinelder.tex"),
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 52,
        model_kind: 0x071c,
        name: "Coin [Prisoner]",
        description: "Large coin imprinted with image of a prisoner.\nFound it in the west apartment building.",
        examine_text: Some("The reverse side shows a nest with eggs."),
        category: ItemCategory::PuzzleItem,
        model_path: Some("data/chr/item/x_coinprisoner.mdl"),
        inspect_texture_path: Some("pic/item/x_coinprisoner.tex"),
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 53,
        model_kind: 0x0708,
        name: "Piece of hair",
        description: "Long, brown hair.\nIt was in that \"box.\"",
        examine_text: None,
        category: ItemCategory::PuzzleItem,
        model_path: None,
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 54,
        model_kind: 0x0709,
        name: "Bent needle",
        description: "Rusty, bent needle. It was stuck in\nthe stuffed animal in the locker room.",
        examine_text: None,
        category: ItemCategory::PuzzleItem,
        model_path: Some("data/chr/item/x_needle.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 55,
        model_kind: 0x072b,
        name: "Dry cell battery",
        description: "Got it in the hospital room.\nJust an ordinary dry cell battery.",
        examine_text: Some("It looks like it's the same type\nof battery that the flashlight uses."),
        category: ItemCategory::PuzzleItem,
        model_path: Some("data/chr/item/x_battery.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 56,
        model_kind: 0x0712,
        name: "Copper ring",
        description: "I found it in the basement of the hospital.\nIt's engraved with a picture of a spider.",
        examine_text: None,
        category: ItemCategory::PuzzleItem,
        model_path: Some("data/chr/item/x_ringcopper.mdl"),
        inspect_texture_path: Some("pic/item/x_ringopper.tex"),
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 57,
        model_kind: 0x0711,
        name: "Lead ring",
        description: "Ring from the refrigerator.\nIt's engraved with a disgusting, bloated face.",
        examine_text: None,
        category: ItemCategory::PuzzleItem,
        model_path: Some("data/chr/item/x_ringlead.mdl"),
        inspect_texture_path: Some("pic/item/x_ringlead.tex"),
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 58,
        model_kind: 0x0710,
        name: "Wrench",
        description: "I found this on a porch on Lindsey Street.\nIt's a tool used for turning nuts and bolts.",
        examine_text: None,
        category: ItemCategory::PuzzleItem,
        model_path: Some("data/chr/item/x_spanner.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 59,
        model_kind: 0x0738,
        name: "Tablet of \"The Oppressor\"",
        description: "Metal tablet with \"The Oppressor\" drawn\non one side. Found it in the cell.",
        examine_text: None,
        category: ItemCategory::PuzzleItem,
        model_path: Some("data/chr/item/x_plate_kick.mdl"),
        inspect_texture_path: Some("pic/item/x_plate_kick.tex"),
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 60,
        model_kind: 0x0737,
        name: "Tablet of \"Gluttonous Pig\"",
        description: "Metal tablet with \"Gluttonous Pig\" drawn\non one side. Found it in the dining room.",
        examine_text: None,
        category: ItemCategory::PuzzleItem,
        model_path: Some("data/chr/item/x_plate_pig.mdl"),
        inspect_texture_path: Some("pic/item/x_plate_pig.tex"),
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 61,
        model_kind: 0x0739,
        name: "Tablet of \"The Seductress\"",
        description: "Metal tablet with \"Seductress\" drawn\non one side. Found it in the shower room.",
        examine_text: None,
        category: ItemCategory::PuzzleItem,
        model_path: Some("data/chr/item/x_plate_female.mdl"),
        inspect_texture_path: Some("pic/item/x_plate_female.tex"),
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 62,
        model_kind: 0x073a,
        name: "Horseshoe",
        description: "Metal horseshoe.\nIt was hanging in the door of the square.",
        examine_text: None,
        category: ItemCategory::PuzzleItem,
        model_path: Some("data/chr/item/x_horse.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 63,
        model_kind: 0x073c,
        name: "Lighter",
        description: "Metal lighter. Looks like its flame\nis stronger than normal lighters.",
        examine_text: None,
        category: ItemCategory::PuzzleItem,
        model_path: Some("data/chr/item/x_lighter.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 64,
        model_kind: 0x070d,
        name: "Wax doll",
        description: "Doll carved from wax.\nIt was lying in the cell.",
        examine_text: None,
        category: ItemCategory::PuzzleItem,
        model_path: Some("data/chr/item/x_waxdoll.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 65,
        model_kind: 0x0714,
        name: "Wire cutter",
        description: "Tool for cutting wires or electric cables.",
        examine_text: None,
        category: ItemCategory::PuzzleItem,
        model_path: Some("data/chr/item/x_plier.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 66,
        model_kind: 0x070f,
        name: "Thinner",
        description: "Small can filled with thinner.\nIt was lying inside the elevator.",
        examine_text: None,
        category: ItemCategory::PuzzleItem,
        model_path: Some("data/chr/item/x_thinner.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 67,
        model_kind: 0x0719,
        name: "\"Little Mermaid\" music box",
        description: "Music box with a figure from the fairy\ntale \"The Little Mermaid\" attached.\nFound at the garden fountain.",
        examine_text: None,
        category: ItemCategory::PuzzleItem,
        model_path: Some("data/chr/item/x_mermaid.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 68,
        model_kind: 0x071a,
        name: "\"Cinderella\" music box",
        description: "Music box with a figure from the\nfairy tale \"Cinderella\" attached.\nFound inside trunk in room 202.",
        examine_text: None,
        category: ItemCategory::PuzzleItem,
        model_path: Some("data/chr/item/x_cinderella.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 69,
        model_kind: 0x0718,
        name: "\"Snow White\" music box",
        description: "Music box with a figure from the\nfairy tale \"Snow White\" attached.\nFound in the pantry.",
        examine_text: None,
        category: ItemCategory::PuzzleItem,
        model_path: Some("data/chr/item/x_snow.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 70,
        model_kind: 0x0720,
        name: "Can opener",
        description: "Tool for opening cans.",
        examine_text: None,
        category: ItemCategory::PuzzleItem,
        model_path: Some("data/chr/item/x_canopen.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 71,
        model_kind: 0x0734,
        name: "Light bulb",
        description: "Found inside can in kitchen of bar.\nNormal light bulb.",
        examine_text: None,
        category: ItemCategory::PuzzleItem,
        model_path: Some("data/chr/item/x_lightbulb.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 72,
        model_kind: 0x0000,
        name: "Rust-colored egg",
        description: "It's about the size of a quail's egg.\nPyramid Head was holding it.",
        examine_text: None,
        category: ItemCategory::PuzzleItem,
        model_path: None,
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 73,
        model_kind: 0x0000,
        name: "Scarlet egg",
        description: "About the size of a quail's egg.\nPyramid Head was holding it.",
        examine_text: None,
        category: ItemCategory::PuzzleItem,
        model_path: None,
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 74,
        model_kind: 0x0721,
        name: "Book: \"Lost Memories\"",
        description: "A book about the history and legends\nof this town and the surrounding area.",
        examine_text: Some("The name comes from the legend\nof the people whose land was\nstolen from them.\x0cThey called this place \"The Place\nof the Silent Spirits.\" By \"Spirits,\"\nthey meant not only their dead\nrelatives, but also the spirits that\nthey believed inhabited the trees,\nrocks and water around them.\x0cAccording to legend, this was\nwhere the holiest ceremonies\ntook place.\x0cBut it was not the ancestors of\nthose who now live in this town\nthat first stole the land from these\npeople. There were others who\ncame before.\x0cIn those days, this town went by\nanother name. But that name is\nnow hopelessly lost in the veils\nof time.\x0cAll we know is that there was\nanother name, and that for some\nreason the town was once\nabandoned by its residents."),
        category: ItemCategory::SecretItem,
        model_path: Some("data/chr/item/x_lostmemory.mdl"),
        inspect_texture_path: Some("pic/item/p_lostmemory.tex"),
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 75,
        model_kind: 0x0713,
        name: "Book: \"Crimson Ceremony\"",
        description: "About some ancient god.\nAuthor unknown.",
        examine_text: Some("Speak.\nI am the Crimson One.\x0cThe lies and the mist are\nnot they but I.\nYou all know that I am One.\nYes, and the One is I.\x0cBelievers hearken to me!\nTwenty score men and\nseven thousand beasts.\nHeed my words and speaketh them\nto all, that they shall ever be\nobeyed even under the light of\nthe proud and merciless sun.\x0cI shall bring down bitter vengeance\nupon thee and thou shalt suffer\nmy eternal wrath.\x0cThe beauty of the withering flower\nand the last struggles of the dying\nman, they are my blessings.\x0cThou shalt ever call upon me and\nall that is me in the place that is\nsilent.\x0cOh, proud fragrance of life which\nflies towards the heart. Oh Cup\nwhich brims with the whitest of\nwine, it is in thee that all begins."),
        category: ItemCategory::SecretItem,
        model_path: Some("data/chr/item/x_redrelig.mdl"),
        inspect_texture_path: Some("pic/item/p_redreling.tex"),
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 76,
        model_kind: 0x0717,
        name: "White chrism",
        description: "Glass bottle containing milky-white anointing oil.",
        examine_text: None,
        category: ItemCategory::SecretItem,
        model_path: Some("data/chr/item/x_oil.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 77,
        model_kind: 0x071e,
        name: "Obsidian goblet",
        description: "Ancient-looking goblet carved from pure\nobsidian. Found it in the old museum.",
        examine_text: None,
        category: ItemCategory::SecretItem,
        model_path: Some("data/chr/item/x_cup.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 78,
        model_kind: 0x074b,
        name: "Blue Gem",
        description: "Maybe I could use it somewhere...",
        examine_text: None,
        category: ItemCategory::SecretItem,
        model_path: Some("data/chr2/item/x_cosmos.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 79,
        model_kind: 0x074e,
        name: "White Board",
        description: "White metal board with square holes\npunched into it. Found in 2nd floor\nEntrance Hall.",
        examine_text: Some("When the White Breath is found,\nI shall dedicate this thing.\nOh Spirit of the Mist,\nGrant us fortune eternal.\x0c... is written on the back."),
        category: ItemCategory::SubScenarioItem,
        model_path: Some("data/chr2/item/x_houban_white.mdl"),
        inspect_texture_path: Some("pic/add/xwhitehouban.tex"),
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 80,
        model_kind: 0x074c,
        name: "Black Board",
        description: "Black metal board with square holes\npunched into it. Found in mansion's\n2nd floor garden.",
        examine_text: Some("When the Dark Grail is found,\nI shall dedicate this thing.\nYou who deny Death,\nGrant us fortune eternal.\x0c... is written on the back."),
        category: ItemCategory::SubScenarioItem,
        model_path: Some("data/chr2/item/x_houban_black.mdl"),
        inspect_texture_path: Some("pic/add/xblackhouban.tex"),
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 81,
        model_kind: 0x074d,
        name: "Red Board",
        description: "Red metal board with square holes\npunched into it. Found in mansion's\n2nd floor garden.",
        examine_text: Some("When the Crimson Words are found,\nI shall dedicate this thing.\nOh you Gods deep in slumber,\nGrant us fortune eternal.\x0c... is written on the back."),
        category: ItemCategory::SubScenarioItem,
        model_path: Some("data/chr2/item/x_houban_red.mdl"),
        inspect_texture_path: Some("pic/add/xredhouban.tex"),
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 82,
        model_kind: 0x074f,
        name: "Acacia key",
        description: "Engraved with an Acacia flower.\nFound embedded in a stone slab.",
        examine_text: None,
        category: ItemCategory::SubScenarioItem,
        model_path: Some("data/chr2/item/x_keyacacia.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 83,
        model_kind: 0x0750,
        name: "Matches",
        description: "Only two or three matches are left.\nFound in kid's room.",
        examine_text: None,
        category: ItemCategory::SubScenarioItem,
        model_path: Some("data/chr2/item/x_mach.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 84,
        model_kind: 0x0748,
        name: "Birthday card and present",
        description: "A birthday card and unopened present from Amy.",
        examine_text: Some("TO MY DEAREST DADDY\nHAPPY BIRTHDAY!\nFROM AMY BALDWIN"),
        category: ItemCategory::SubScenarioItem,
        model_path: Some("data/chr2/item/x_birthday.mdl"),
        inspect_texture_path: Some("pic/add/p_birthday.tex"),
        base_pickup_amount: 1,
    },
    ItemMetadata {
        id: 85,
        model_kind: 0x0717,
        name: "White liquid",
        description: "Small bottle filled with an unknown\nwhite liquid. The label reads: \"I deny\nthee.\"",
        examine_text: None,
        category: ItemCategory::SubScenarioItem,
        model_path: Some("data/chr/item/x_oil.mdl"),
        inspect_texture_path: None,
        base_pickup_amount: 1,
    },
];

/// Looks up static catalog metadata for item ID `1..=85`.
pub fn item_metadata(item_id: u8) -> Option<&'static ItemMetadata> {
    if (1..=85).contains(&item_id) {
        Some(&ITEM_CATALOG[(item_id - 1) as usize])
    } else {
        None
    }
}

/// Evaluates whether 2 or 3 item IDs (`a`, `b`, `c`, where `0` represents an empty slot)
/// combine into one of the 11 combination pseudo-items (`86..=96`, `0x49ea00`).
pub fn combine_items(a: u8, b: u8, c: u8) -> Option<u8> {
    let mut v = [a, b, c];
    v.sort_unstable();
    // Deduplicate non-zero duplicates (cannot combine an item with itself)
    if (v[0] != 0 && v[0] == v[1]) || (v[1] != 0 && v[1] == v[2]) {
        return None;
    }
    ITEM_COMBOS
        .iter()
        .find(|combo| combo.parts == v)
        .map(|combo| combo.result_id)
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
