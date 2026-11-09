//! Static PS2 stage event lookup tables (`ITEM_KIND_TABLE`, `DOOR_SE_TABLE`, `CONNECT_STAGE_PAIRS`).

/// 8-byte pickup kind descriptor (`item_kind[8]` at `SLUS_202.28` `0x002b97e0`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemKindEntry {
    /// Inventory item ID granted by `ItemGet`.
    pub item_id: i16,
    /// Common `.mes` message ID displayed on pickup.
    pub msg_id: u16,
    /// 3D character/prop model ID (`0x4xx`) deleted when picked up.
    pub chr_id: i32,
}

/// Canonical PS2 `item_kind[8]` pickup table (`SLUS_202.28` `0x002b97e0`).
pub const ITEM_KIND_TABLE: [ItemKindEntry; 8] = [
    ItemKindEntry {
        item_id: 1,
        msg_id: 10,
        chr_id: 0x0401,
    }, // Health drink
    ItemKindEntry {
        item_id: 2,
        msg_id: 11,
        chr_id: 0x0402,
    }, // First-aid kit
    ItemKindEntry {
        item_id: 3,
        msg_id: 12,
        chr_id: 0x0403,
    }, // Ampoule
    ItemKindEntry {
        item_id: 5,
        msg_id: 13,
        chr_id: 0x0405,
    }, // Handgun bullets
    ItemKindEntry {
        item_id: 7,
        msg_id: 14,
        chr_id: 0x0407,
    }, // Shotgun shells
    ItemKindEntry {
        item_id: 9,
        msg_id: 15,
        chr_id: 0x0409,
    }, // Rifle shells
    ItemKindEntry {
        item_id: 11,
        msg_id: 50,
        chr_id: 0x040b,
    }, // Revolver bullets (Born from a Wish)
    ItemKindEntry {
        item_id: 0,
        msg_id: 0,
        chr_id: 0,
    }, // Sentinel
];

/// 12-byte door sound effect entry (`door_se[22]` at `SLUS_202.28` `0x002b96d0`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventDoorSound {
    pub open: i16,
    pub close: i16,
    pub lock: i16,
    pub unlock: i16,
    pub broken: i16,
    pub pad: i16,
}

/// Canonical PS2 `door_se[22]` table (`SLUS_202.28` `0x002b96d0`).
pub const DOOR_SE_TABLE: [EventDoorSound; 22] = [
    EventDoorSound {
        open: 0,
        close: 0,
        lock: 0,
        unlock: 0,
        broken: 0,
        pad: 0,
    },
    EventDoorSound {
        open: 11000,
        close: 11001,
        lock: 11002,
        unlock: 11003,
        broken: 11004,
        pad: 0,
    },
    EventDoorSound {
        open: 11092,
        close: 11093,
        lock: 11094,
        unlock: 11095,
        broken: 11096,
        pad: 0,
    },
    EventDoorSound {
        open: 11097,
        close: 11098,
        lock: 11099,
        unlock: 11100,
        broken: 11101,
        pad: 0,
    },
    EventDoorSound {
        open: 11056,
        close: 11057,
        lock: 11058,
        unlock: 11059,
        broken: 11060,
        pad: 0,
    },
    EventDoorSound {
        open: 11061,
        close: 11062,
        lock: 11063,
        unlock: 11064,
        broken: 11065,
        pad: 0,
    },
    EventDoorSound {
        open: 11066,
        close: 11067,
        lock: 11068,
        unlock: 11069,
        broken: 11070,
        pad: 0,
    },
    EventDoorSound {
        open: 11071,
        close: 11072,
        lock: 11073,
        unlock: 11074,
        broken: 11075,
        pad: 0,
    },
    EventDoorSound {
        open: 11076,
        close: 11077,
        lock: 11078,
        unlock: 11079,
        broken: 11080,
        pad: 0,
    },
    EventDoorSound {
        open: 11081,
        close: 11082,
        lock: 11083,
        unlock: 11084,
        broken: 11085,
        pad: 0,
    },
    EventDoorSound {
        open: 11086,
        close: 11087,
        lock: 11088,
        unlock: 11089,
        broken: 11090,
        pad: 0,
    },
    EventDoorSound {
        open: 11091,
        close: 0,
        lock: 0,
        unlock: 0,
        broken: 0,
        pad: 0,
    },
    EventDoorSound {
        open: 11102,
        close: 0,
        lock: 0,
        unlock: 0,
        broken: 0,
        pad: 0,
    },
    EventDoorSound {
        open: 11103,
        close: 11104,
        lock: 11105,
        unlock: 11106,
        broken: 11107,
        pad: 0,
    },
    EventDoorSound {
        open: 11108,
        close: 11109,
        lock: 11110,
        unlock: 11111,
        broken: 11112,
        pad: 0,
    },
    EventDoorSound {
        open: 11113,
        close: 11114,
        lock: 11115,
        unlock: 11116,
        broken: 11117,
        pad: 0,
    },
    EventDoorSound {
        open: 11118,
        close: 11119,
        lock: 11120,
        unlock: 11121,
        broken: 11122,
        pad: 0,
    },
    EventDoorSound {
        open: 11123,
        close: 11124,
        lock: 11125,
        unlock: 11126,
        broken: 11127,
        pad: 0,
    },
    EventDoorSound {
        open: 11128,
        close: 11129,
        lock: 11130,
        unlock: 11131,
        broken: 11132,
        pad: 0,
    },
    EventDoorSound {
        open: 11133,
        close: 11134,
        lock: 11135,
        unlock: 11136,
        broken: 11137,
        pad: 0,
    },
    EventDoorSound {
        open: 11138,
        close: 11139,
        lock: 11140,
        unlock: 11141,
        broken: 11142,
        pad: 0,
    },
    EventDoorSound {
        open: 0,
        close: 0,
        lock: 0,
        unlock: 0,
        broken: 0,
        pad: 0,
    },
];

/// Connected stage pairs (`SLUS_202.28` `0x002b9820`) that share seamless sub-stage assets
/// during `EventExecMove` (setting transition flag bit `0x08`).
pub const CONNECT_STAGE_PAIRS: [(i16, i16); 11] = [
    (6, 7),   // Apart E1F <-> E2F
    (8, 9),   // Apart E3FW <-> E3FE
    (10, 11), // Apart W1F <-> W2F
    (16, 17), // Hospital 1F <-> 2F
    (18, 19), // Hospital 3F <-> RF
    (21, 22), // Hospital B1F <-> B2F
    (29, 30), // Labyrinth W <-> E
    (36, 37), // Hotel 1F <-> 2F
    (38, 39), // Hotel 3F <-> B1F
    (42, 43), // Hotel Fire 1F <-> 2F
    (0, 0),   // Sentinel
];
