//! Static 2D↔3D sound ID mapping table (`change_list`) extracted 1:1 from `sh2pc.exe` (`0x008a4b90`).
//!
//! # Prototype (`SLUS_202.28`) vs. Retail PC (`sh2pc.exe`) Drift
//! - Both `SLUS_202.28` (`0x002b8880`) and `sh2pc.exe` (`0x008a4b90`) contain 241 null-terminated
//!   `(id_2d, id_3d, bank_id)` triples (12 bytes each).
//! - `sh2pc.exe` shifted `bank_id` and `id_3d` indices to match the retail `sddata.bin` layout
//!   (spanning banks `1..=36`, including *Born from a Wish*).

use super::types::Se2d3dMapping;

/// Retail `change_list` table (`sh2pc.exe` `0x008a4b90`, 241 entries).
pub const SE_CHANGE_LIST: [Se2d3dMapping; 241] = [
    Se2d3dMapping {
        id_2d: 12009,
        id_3d: 40245,
        bank_id: 2,
    },
    Se2d3dMapping {
        id_2d: 12019,
        id_3d: 40244,
        bank_id: 2,
    },
    Se2d3dMapping {
        id_2d: 12200,
        id_3d: 40248,
        bank_id: 2,
    },
    Se2d3dMapping {
        id_2d: 12201,
        id_3d: 40249,
        bank_id: 2,
    },
    Se2d3dMapping {
        id_2d: 12208,
        id_3d: 40246,
        bank_id: 2,
    },
    Se2d3dMapping {
        id_2d: 12209,
        id_3d: 40247,
        bank_id: 2,
    },
    Se2d3dMapping {
        id_2d: 12500,
        id_3d: 40250,
        bank_id: 3,
    },
    Se2d3dMapping {
        id_2d: 16100,
        id_3d: 40255,
        bank_id: 3,
    },
    Se2d3dMapping {
        id_2d: 16102,
        id_3d: 40256,
        bank_id: 3,
    },
    Se2d3dMapping {
        id_2d: 16104,
        id_3d: 40257,
        bank_id: 3,
    },
    Se2d3dMapping {
        id_2d: 16113,
        id_3d: 40258,
        bank_id: 3,
    },
    Se2d3dMapping {
        id_2d: 16118,
        id_3d: 40259,
        bank_id: 3,
    },
    Se2d3dMapping {
        id_2d: 16119,
        id_3d: 40260,
        bank_id: 3,
    },
    Se2d3dMapping {
        id_2d: 12100,
        id_3d: 40252,
        bank_id: 3,
    },
    Se2d3dMapping {
        id_2d: 12103,
        id_3d: 40253,
        bank_id: 3,
    },
    Se2d3dMapping {
        id_2d: 12107,
        id_3d: 40254,
        bank_id: 3,
    },
    Se2d3dMapping {
        id_2d: 12500,
        id_3d: 40261,
        bank_id: 4,
    },
    Se2d3dMapping {
        id_2d: 16100,
        id_3d: 40265,
        bank_id: 4,
    },
    Se2d3dMapping {
        id_2d: 16102,
        id_3d: 40266,
        bank_id: 4,
    },
    Se2d3dMapping {
        id_2d: 16104,
        id_3d: 40267,
        bank_id: 4,
    },
    Se2d3dMapping {
        id_2d: 16113,
        id_3d: 40268,
        bank_id: 4,
    },
    Se2d3dMapping {
        id_2d: 16118,
        id_3d: 40269,
        bank_id: 4,
    },
    Se2d3dMapping {
        id_2d: 16119,
        id_3d: 40270,
        bank_id: 4,
    },
    Se2d3dMapping {
        id_2d: 12009,
        id_3d: 40264,
        bank_id: 4,
    },
    Se2d3dMapping {
        id_2d: 12019,
        id_3d: 40263,
        bank_id: 4,
    },
    Se2d3dMapping {
        id_2d: 16100,
        id_3d: 40276,
        bank_id: 5,
    },
    Se2d3dMapping {
        id_2d: 16102,
        id_3d: 40277,
        bank_id: 5,
    },
    Se2d3dMapping {
        id_2d: 16104,
        id_3d: 40278,
        bank_id: 5,
    },
    Se2d3dMapping {
        id_2d: 16113,
        id_3d: 40279,
        bank_id: 5,
    },
    Se2d3dMapping {
        id_2d: 16118,
        id_3d: 40280,
        bank_id: 5,
    },
    Se2d3dMapping {
        id_2d: 16119,
        id_3d: 40281,
        bank_id: 5,
    },
    Se2d3dMapping {
        id_2d: 12100,
        id_3d: 40273,
        bank_id: 5,
    },
    Se2d3dMapping {
        id_2d: 12103,
        id_3d: 40274,
        bank_id: 5,
    },
    Se2d3dMapping {
        id_2d: 12107,
        id_3d: 40275,
        bank_id: 5,
    },
    Se2d3dMapping {
        id_2d: 12250,
        id_3d: 40271,
        bank_id: 5,
    },
    Se2d3dMapping {
        id_2d: 12251,
        id_3d: 40272,
        bank_id: 5,
    },
    Se2d3dMapping {
        id_2d: 12200,
        id_3d: 40005,
        bank_id: 6,
    },
    Se2d3dMapping {
        id_2d: 12201,
        id_3d: 40006,
        bank_id: 6,
    },
    Se2d3dMapping {
        id_2d: 12208,
        id_3d: 40003,
        bank_id: 6,
    },
    Se2d3dMapping {
        id_2d: 12209,
        id_3d: 40004,
        bank_id: 6,
    },
    Se2d3dMapping {
        id_2d: 12009,
        id_3d: 40002,
        bank_id: 6,
    },
    Se2d3dMapping {
        id_2d: 12019,
        id_3d: 40001,
        bank_id: 6,
    },
    Se2d3dMapping {
        id_2d: 12009,
        id_3d: 40012,
        bank_id: 7,
    },
    Se2d3dMapping {
        id_2d: 12019,
        id_3d: 40011,
        bank_id: 7,
    },
    Se2d3dMapping {
        id_2d: 12200,
        id_3d: 40015,
        bank_id: 7,
    },
    Se2d3dMapping {
        id_2d: 12201,
        id_3d: 40016,
        bank_id: 7,
    },
    Se2d3dMapping {
        id_2d: 12208,
        id_3d: 40013,
        bank_id: 7,
    },
    Se2d3dMapping {
        id_2d: 12209,
        id_3d: 40014,
        bank_id: 7,
    },
    Se2d3dMapping {
        id_2d: 12500,
        id_3d: 40010,
        bank_id: 7,
    },
    Se2d3dMapping {
        id_2d: 12200,
        id_3d: 40023,
        bank_id: 8,
    },
    Se2d3dMapping {
        id_2d: 12201,
        id_3d: 40024,
        bank_id: 8,
    },
    Se2d3dMapping {
        id_2d: 12208,
        id_3d: 40021,
        bank_id: 8,
    },
    Se2d3dMapping {
        id_2d: 12209,
        id_3d: 40022,
        bank_id: 8,
    },
    Se2d3dMapping {
        id_2d: 12009,
        id_3d: 40020,
        bank_id: 8,
    },
    Se2d3dMapping {
        id_2d: 12019,
        id_3d: 40019,
        bank_id: 8,
    },
    Se2d3dMapping {
        id_2d: 12500,
        id_3d: 40018,
        bank_id: 8,
    },
    Se2d3dMapping {
        id_2d: 16100,
        id_3d: 40039,
        bank_id: 9,
    },
    Se2d3dMapping {
        id_2d: 16102,
        id_3d: 40040,
        bank_id: 9,
    },
    Se2d3dMapping {
        id_2d: 16104,
        id_3d: 40041,
        bank_id: 9,
    },
    Se2d3dMapping {
        id_2d: 16113,
        id_3d: 40042,
        bank_id: 9,
    },
    Se2d3dMapping {
        id_2d: 16118,
        id_3d: 40043,
        bank_id: 9,
    },
    Se2d3dMapping {
        id_2d: 12500,
        id_3d: 40034,
        bank_id: 9,
    },
    Se2d3dMapping {
        id_2d: 12200,
        id_3d: 40037,
        bank_id: 9,
    },
    Se2d3dMapping {
        id_2d: 12201,
        id_3d: 40038,
        bank_id: 9,
    },
    Se2d3dMapping {
        id_2d: 12208,
        id_3d: 40035,
        bank_id: 9,
    },
    Se2d3dMapping {
        id_2d: 12209,
        id_3d: 40036,
        bank_id: 9,
    },
    Se2d3dMapping {
        id_2d: 12500,
        id_3d: 40045,
        bank_id: 10,
    },
    Se2d3dMapping {
        id_2d: 12009,
        id_3d: 40047,
        bank_id: 10,
    },
    Se2d3dMapping {
        id_2d: 12019,
        id_3d: 40046,
        bank_id: 10,
    },
    Se2d3dMapping {
        id_2d: 12500,
        id_3d: 40025,
        bank_id: 11,
    },
    Se2d3dMapping {
        id_2d: 12009,
        id_3d: 40027,
        bank_id: 11,
    },
    Se2d3dMapping {
        id_2d: 12019,
        id_3d: 40026,
        bank_id: 11,
    },
    Se2d3dMapping {
        id_2d: 16100,
        id_3d: 40028,
        bank_id: 11,
    },
    Se2d3dMapping {
        id_2d: 16102,
        id_3d: 40029,
        bank_id: 11,
    },
    Se2d3dMapping {
        id_2d: 16104,
        id_3d: 40030,
        bank_id: 11,
    },
    Se2d3dMapping {
        id_2d: 16113,
        id_3d: 40031,
        bank_id: 11,
    },
    Se2d3dMapping {
        id_2d: 16118,
        id_3d: 40032,
        bank_id: 11,
    },
    Se2d3dMapping {
        id_2d: 16119,
        id_3d: 40033,
        bank_id: 11,
    },
    Se2d3dMapping {
        id_2d: 12500,
        id_3d: 40084,
        bank_id: 12,
    },
    Se2d3dMapping {
        id_2d: 16100,
        id_3d: 40088,
        bank_id: 12,
    },
    Se2d3dMapping {
        id_2d: 16102,
        id_3d: 40089,
        bank_id: 12,
    },
    Se2d3dMapping {
        id_2d: 16104,
        id_3d: 40090,
        bank_id: 12,
    },
    Se2d3dMapping {
        id_2d: 16113,
        id_3d: 40091,
        bank_id: 12,
    },
    Se2d3dMapping {
        id_2d: 16118,
        id_3d: 40092,
        bank_id: 12,
    },
    Se2d3dMapping {
        id_2d: 16119,
        id_3d: 40093,
        bank_id: 12,
    },
    Se2d3dMapping {
        id_2d: 12100,
        id_3d: 40085,
        bank_id: 12,
    },
    Se2d3dMapping {
        id_2d: 12103,
        id_3d: 40086,
        bank_id: 12,
    },
    Se2d3dMapping {
        id_2d: 12107,
        id_3d: 40087,
        bank_id: 12,
    },
    Se2d3dMapping {
        id_2d: 12500,
        id_3d: 40105,
        bank_id: 13,
    },
    Se2d3dMapping {
        id_2d: 16100,
        id_3d: 40109,
        bank_id: 13,
    },
    Se2d3dMapping {
        id_2d: 16102,
        id_3d: 40110,
        bank_id: 13,
    },
    Se2d3dMapping {
        id_2d: 16104,
        id_3d: 40111,
        bank_id: 13,
    },
    Se2d3dMapping {
        id_2d: 16113,
        id_3d: 40112,
        bank_id: 13,
    },
    Se2d3dMapping {
        id_2d: 16118,
        id_3d: 40113,
        bank_id: 13,
    },
    Se2d3dMapping {
        id_2d: 16119,
        id_3d: 40114,
        bank_id: 13,
    },
    Se2d3dMapping {
        id_2d: 12100,
        id_3d: 40106,
        bank_id: 13,
    },
    Se2d3dMapping {
        id_2d: 12103,
        id_3d: 40107,
        bank_id: 13,
    },
    Se2d3dMapping {
        id_2d: 12107,
        id_3d: 40108,
        bank_id: 13,
    },
    Se2d3dMapping {
        id_2d: 12500,
        id_3d: 40125,
        bank_id: 14,
    },
    Se2d3dMapping {
        id_2d: 16100,
        id_3d: 40129,
        bank_id: 14,
    },
    Se2d3dMapping {
        id_2d: 16102,
        id_3d: 40130,
        bank_id: 14,
    },
    Se2d3dMapping {
        id_2d: 16104,
        id_3d: 40131,
        bank_id: 14,
    },
    Se2d3dMapping {
        id_2d: 16113,
        id_3d: 40132,
        bank_id: 14,
    },
    Se2d3dMapping {
        id_2d: 16118,
        id_3d: 40133,
        bank_id: 14,
    },
    Se2d3dMapping {
        id_2d: 16119,
        id_3d: 40134,
        bank_id: 14,
    },
    Se2d3dMapping {
        id_2d: 12100,
        id_3d: 40126,
        bank_id: 14,
    },
    Se2d3dMapping {
        id_2d: 12103,
        id_3d: 40127,
        bank_id: 14,
    },
    Se2d3dMapping {
        id_2d: 12107,
        id_3d: 40128,
        bank_id: 14,
    },
    Se2d3dMapping {
        id_2d: 12500,
        id_3d: 40151,
        bank_id: 15,
    },
    Se2d3dMapping {
        id_2d: 16100,
        id_3d: 40155,
        bank_id: 15,
    },
    Se2d3dMapping {
        id_2d: 16102,
        id_3d: 40156,
        bank_id: 15,
    },
    Se2d3dMapping {
        id_2d: 16104,
        id_3d: 40157,
        bank_id: 15,
    },
    Se2d3dMapping {
        id_2d: 16113,
        id_3d: 40158,
        bank_id: 15,
    },
    Se2d3dMapping {
        id_2d: 16118,
        id_3d: 40159,
        bank_id: 15,
    },
    Se2d3dMapping {
        id_2d: 16119,
        id_3d: 40160,
        bank_id: 15,
    },
    Se2d3dMapping {
        id_2d: 12100,
        id_3d: 40152,
        bank_id: 15,
    },
    Se2d3dMapping {
        id_2d: 12103,
        id_3d: 40153,
        bank_id: 15,
    },
    Se2d3dMapping {
        id_2d: 12107,
        id_3d: 40154,
        bank_id: 15,
    },
    Se2d3dMapping {
        id_2d: 18500,
        id_3d: 40197,
        bank_id: 16,
    },
    Se2d3dMapping {
        id_2d: 18512,
        id_3d: 40198,
        bank_id: 16,
    },
    Se2d3dMapping {
        id_2d: 18508,
        id_3d: 40196,
        bank_id: 16,
    },
    Se2d3dMapping {
        id_2d: 16100,
        id_3d: 40078,
        bank_id: 17,
    },
    Se2d3dMapping {
        id_2d: 16102,
        id_3d: 40079,
        bank_id: 17,
    },
    Se2d3dMapping {
        id_2d: 16104,
        id_3d: 40080,
        bank_id: 17,
    },
    Se2d3dMapping {
        id_2d: 16113,
        id_3d: 40081,
        bank_id: 17,
    },
    Se2d3dMapping {
        id_2d: 16118,
        id_3d: 40082,
        bank_id: 17,
    },
    Se2d3dMapping {
        id_2d: 16119,
        id_3d: 40083,
        bank_id: 17,
    },
    Se2d3dMapping {
        id_2d: 12100,
        id_3d: 40075,
        bank_id: 17,
    },
    Se2d3dMapping {
        id_2d: 12103,
        id_3d: 40076,
        bank_id: 17,
    },
    Se2d3dMapping {
        id_2d: 12107,
        id_3d: 40077,
        bank_id: 17,
    },
    Se2d3dMapping {
        id_2d: 16100,
        id_3d: 40097,
        bank_id: 18,
    },
    Se2d3dMapping {
        id_2d: 16102,
        id_3d: 40098,
        bank_id: 18,
    },
    Se2d3dMapping {
        id_2d: 16104,
        id_3d: 40099,
        bank_id: 18,
    },
    Se2d3dMapping {
        id_2d: 16113,
        id_3d: 40100,
        bank_id: 18,
    },
    Se2d3dMapping {
        id_2d: 16118,
        id_3d: 40101,
        bank_id: 18,
    },
    Se2d3dMapping {
        id_2d: 16119,
        id_3d: 40102,
        bank_id: 18,
    },
    Se2d3dMapping {
        id_2d: 12100,
        id_3d: 40094,
        bank_id: 18,
    },
    Se2d3dMapping {
        id_2d: 12103,
        id_3d: 40095,
        bank_id: 18,
    },
    Se2d3dMapping {
        id_2d: 12107,
        id_3d: 40096,
        bank_id: 18,
    },
    Se2d3dMapping {
        id_2d: 16100,
        id_3d: 40119,
        bank_id: 19,
    },
    Se2d3dMapping {
        id_2d: 16102,
        id_3d: 40120,
        bank_id: 19,
    },
    Se2d3dMapping {
        id_2d: 16104,
        id_3d: 40121,
        bank_id: 19,
    },
    Se2d3dMapping {
        id_2d: 16113,
        id_3d: 40122,
        bank_id: 19,
    },
    Se2d3dMapping {
        id_2d: 16118,
        id_3d: 40123,
        bank_id: 19,
    },
    Se2d3dMapping {
        id_2d: 16119,
        id_3d: 40124,
        bank_id: 19,
    },
    Se2d3dMapping {
        id_2d: 12100,
        id_3d: 40116,
        bank_id: 19,
    },
    Se2d3dMapping {
        id_2d: 12103,
        id_3d: 40117,
        bank_id: 19,
    },
    Se2d3dMapping {
        id_2d: 12107,
        id_3d: 40118,
        bank_id: 19,
    },
    Se2d3dMapping {
        id_2d: 16100,
        id_3d: 40139,
        bank_id: 20,
    },
    Se2d3dMapping {
        id_2d: 16102,
        id_3d: 40140,
        bank_id: 20,
    },
    Se2d3dMapping {
        id_2d: 16104,
        id_3d: 40141,
        bank_id: 20,
    },
    Se2d3dMapping {
        id_2d: 16113,
        id_3d: 40142,
        bank_id: 20,
    },
    Se2d3dMapping {
        id_2d: 16118,
        id_3d: 40143,
        bank_id: 20,
    },
    Se2d3dMapping {
        id_2d: 16119,
        id_3d: 40144,
        bank_id: 20,
    },
    Se2d3dMapping {
        id_2d: 12100,
        id_3d: 40136,
        bank_id: 20,
    },
    Se2d3dMapping {
        id_2d: 12103,
        id_3d: 40137,
        bank_id: 20,
    },
    Se2d3dMapping {
        id_2d: 12107,
        id_3d: 40138,
        bank_id: 20,
    },
    Se2d3dMapping {
        id_2d: 16100,
        id_3d: 40145,
        bank_id: 21,
    },
    Se2d3dMapping {
        id_2d: 16102,
        id_3d: 40146,
        bank_id: 21,
    },
    Se2d3dMapping {
        id_2d: 16104,
        id_3d: 40147,
        bank_id: 21,
    },
    Se2d3dMapping {
        id_2d: 16113,
        id_3d: 40148,
        bank_id: 21,
    },
    Se2d3dMapping {
        id_2d: 16118,
        id_3d: 40149,
        bank_id: 21,
    },
    Se2d3dMapping {
        id_2d: 16119,
        id_3d: 40150,
        bank_id: 21,
    },
    Se2d3dMapping {
        id_2d: 12009,
        id_3d: 40050,
        bank_id: 22,
    },
    Se2d3dMapping {
        id_2d: 12019,
        id_3d: 40049,
        bank_id: 22,
    },
    Se2d3dMapping {
        id_2d: 12200,
        id_3d: 40054,
        bank_id: 22,
    },
    Se2d3dMapping {
        id_2d: 12201,
        id_3d: 40055,
        bank_id: 22,
    },
    Se2d3dMapping {
        id_2d: 12208,
        id_3d: 40052,
        bank_id: 22,
    },
    Se2d3dMapping {
        id_2d: 12209,
        id_3d: 40053,
        bank_id: 22,
    },
    Se2d3dMapping {
        id_2d: 12009,
        id_3d: 40058,
        bank_id: 22,
    },
    Se2d3dMapping {
        id_2d: 12019,
        id_3d: 40057,
        bank_id: 22,
    },
    Se2d3dMapping {
        id_2d: 12200,
        id_3d: 40062,
        bank_id: 22,
    },
    Se2d3dMapping {
        id_2d: 12201,
        id_3d: 40063,
        bank_id: 22,
    },
    Se2d3dMapping {
        id_2d: 12208,
        id_3d: 40060,
        bank_id: 22,
    },
    Se2d3dMapping {
        id_2d: 12209,
        id_3d: 40061,
        bank_id: 22,
    },
    Se2d3dMapping {
        id_2d: 12009,
        id_3d: 40234,
        bank_id: 24,
    },
    Se2d3dMapping {
        id_2d: 12019,
        id_3d: 40233,
        bank_id: 24,
    },
    Se2d3dMapping {
        id_2d: 12200,
        id_3d: 40237,
        bank_id: 24,
    },
    Se2d3dMapping {
        id_2d: 12201,
        id_3d: 40238,
        bank_id: 24,
    },
    Se2d3dMapping {
        id_2d: 12208,
        id_3d: 40235,
        bank_id: 24,
    },
    Se2d3dMapping {
        id_2d: 12209,
        id_3d: 40236,
        bank_id: 24,
    },
    Se2d3dMapping {
        id_2d: 16100,
        id_3d: 40239,
        bank_id: 24,
    },
    Se2d3dMapping {
        id_2d: 16102,
        id_3d: 40240,
        bank_id: 24,
    },
    Se2d3dMapping {
        id_2d: 16104,
        id_3d: 40241,
        bank_id: 24,
    },
    Se2d3dMapping {
        id_2d: 16113,
        id_3d: 40242,
        bank_id: 24,
    },
    Se2d3dMapping {
        id_2d: 16118,
        id_3d: 40243,
        bank_id: 24,
    },
    Se2d3dMapping {
        id_2d: 12250,
        id_3d: 40217,
        bank_id: 25,
    },
    Se2d3dMapping {
        id_2d: 12251,
        id_3d: 40218,
        bank_id: 25,
    },
    Se2d3dMapping {
        id_2d: 12009,
        id_3d: 40220,
        bank_id: 25,
    },
    Se2d3dMapping {
        id_2d: 12019,
        id_3d: 40219,
        bank_id: 25,
    },
    Se2d3dMapping {
        id_2d: 12200,
        id_3d: 40223,
        bank_id: 25,
    },
    Se2d3dMapping {
        id_2d: 12201,
        id_3d: 40224,
        bank_id: 25,
    },
    Se2d3dMapping {
        id_2d: 12208,
        id_3d: 40221,
        bank_id: 25,
    },
    Se2d3dMapping {
        id_2d: 12209,
        id_3d: 40222,
        bank_id: 25,
    },
    Se2d3dMapping {
        id_2d: 18300,
        id_3d: 40201,
        bank_id: 26,
    },
    Se2d3dMapping {
        id_2d: 12009,
        id_3d: 40200,
        bank_id: 26,
    },
    Se2d3dMapping {
        id_2d: 12019,
        id_3d: 40199,
        bank_id: 26,
    },
    Se2d3dMapping {
        id_2d: 16100,
        id_3d: 40202,
        bank_id: 26,
    },
    Se2d3dMapping {
        id_2d: 16102,
        id_3d: 40203,
        bank_id: 26,
    },
    Se2d3dMapping {
        id_2d: 16104,
        id_3d: 40204,
        bank_id: 26,
    },
    Se2d3dMapping {
        id_2d: 16113,
        id_3d: 40205,
        bank_id: 26,
    },
    Se2d3dMapping {
        id_2d: 16118,
        id_3d: 40206,
        bank_id: 26,
    },
    Se2d3dMapping {
        id_2d: 16119,
        id_3d: 40207,
        bank_id: 26,
    },
    Se2d3dMapping {
        id_2d: 12009,
        id_3d: 40210,
        bank_id: 27,
    },
    Se2d3dMapping {
        id_2d: 12019,
        id_3d: 40209,
        bank_id: 27,
    },
    Se2d3dMapping {
        id_2d: 16100,
        id_3d: 40211,
        bank_id: 27,
    },
    Se2d3dMapping {
        id_2d: 16102,
        id_3d: 40212,
        bank_id: 27,
    },
    Se2d3dMapping {
        id_2d: 16104,
        id_3d: 40213,
        bank_id: 27,
    },
    Se2d3dMapping {
        id_2d: 16113,
        id_3d: 40214,
        bank_id: 27,
    },
    Se2d3dMapping {
        id_2d: 16118,
        id_3d: 40215,
        bank_id: 27,
    },
    Se2d3dMapping {
        id_2d: 16119,
        id_3d: 40216,
        bank_id: 27,
    },
    Se2d3dMapping {
        id_2d: 18200,
        id_3d: 40064,
        bank_id: 28,
    },
    Se2d3dMapping {
        id_2d: 18300,
        id_3d: 40194,
        bank_id: 29,
    },
    Se2d3dMapping {
        id_2d: 18307,
        id_3d: 40195,
        bank_id: 29,
    },
    Se2d3dMapping {
        id_2d: 12500,
        id_3d: 40191,
        bank_id: 29,
    },
    Se2d3dMapping {
        id_2d: 18300,
        id_3d: 40170,
        bank_id: 30,
    },
    Se2d3dMapping {
        id_2d: 18307,
        id_3d: 40171,
        bank_id: 30,
    },
    Se2d3dMapping {
        id_2d: 12500,
        id_3d: 40167,
        bank_id: 30,
    },
    Se2d3dMapping {
        id_2d: 18300,
        id_3d: 40177,
        bank_id: 31,
    },
    Se2d3dMapping {
        id_2d: 18307,
        id_3d: 40178,
        bank_id: 31,
    },
    Se2d3dMapping {
        id_2d: 12500,
        id_3d: 40176,
        bank_id: 31,
    },
    Se2d3dMapping {
        id_2d: 16113,
        id_3d: 40179,
        bank_id: 31,
    },
    Se2d3dMapping {
        id_2d: 16118,
        id_3d: 40180,
        bank_id: 31,
    },
    Se2d3dMapping {
        id_2d: 16119,
        id_3d: 40181,
        bank_id: 31,
    },
    Se2d3dMapping {
        id_2d: 18300,
        id_3d: 40188,
        bank_id: 33,
    },
    Se2d3dMapping {
        id_2d: 18307,
        id_3d: 40189,
        bank_id: 33,
    },
    Se2d3dMapping {
        id_2d: 12500,
        id_3d: 40185,
        bank_id: 33,
    },
    Se2d3dMapping {
        id_2d: 16113,
        id_3d: 40164,
        bank_id: 34,
    },
    Se2d3dMapping {
        id_2d: 16118,
        id_3d: 40165,
        bank_id: 34,
    },
    Se2d3dMapping {
        id_2d: 16119,
        id_3d: 40166,
        bank_id: 34,
    },
    Se2d3dMapping {
        id_2d: 12500,
        id_3d: 40163,
        bank_id: 34,
    },
    Se2d3dMapping {
        id_2d: 12250,
        id_3d: 40161,
        bank_id: 34,
    },
    Se2d3dMapping {
        id_2d: 12251,
        id_3d: 40162,
        bank_id: 34,
    },
    Se2d3dMapping {
        id_2d: 18300,
        id_3d: 40173,
        bank_id: 35,
    },
    Se2d3dMapping {
        id_2d: 18307,
        id_3d: 40174,
        bank_id: 35,
    },
    Se2d3dMapping {
        id_2d: 12500,
        id_3d: 40172,
        bank_id: 35,
    },
    Se2d3dMapping {
        id_2d: 18850,
        id_3d: 40067,
        bank_id: 36,
    },
    Se2d3dMapping {
        id_2d: 18851,
        id_3d: 40068,
        bank_id: 36,
    },
    Se2d3dMapping {
        id_2d: 18858,
        id_3d: 40065,
        bank_id: 36,
    },
    Se2d3dMapping {
        id_2d: 18859,
        id_3d: 40066,
        bank_id: 36,
    },
    Se2d3dMapping {
        id_2d: 18862,
        id_3d: 40069,
        bank_id: 36,
    },
];

/// Resolves the active 3D SFX bank ID (`1..=36`, or `0` if none) for `(stage_id, room_id, is_night)`.
///
/// Ported from `SeSoundEffect3dLoad` (`SLUS_202.28` `0x00192fd0`) with retail PC `sh2pc.exe`
/// `change_list` (`0x008a4b90`) bank indices (`2..=36`):
/// - Prototype `SLUS_202.28` mapped Town East/West -> `1..=4`, Apartments -> `5..=10`,
///   Hospital -> `11..=20`, Delusion/Prison/Labyrinth/Eddie -> `21..=27`, Hotel -> `28..=33`,
///   Endings -> `34`.
/// - Retail `sh2pc.exe` (`0x008a4b90`) shifted those bank IDs (`2..=36`) to align with retail
///   `sddata.bin` and *Born from a Wish* (`Stg_maria_town` / `Stg_maria_apart`).
pub const fn se_3d_bank_for_stage(stage_id: u32, room_id: u32, is_night: bool) -> u32 {
    match stage_id {
        5 => {
            if is_night {
                3
            } else {
                2
            }
        }
        6 => 6,
        7 => 7,
        8 | 9 => 8,
        10 => 9,
        11 => 10,
        12 => 11,
        14 => {
            if is_night {
                5
            } else {
                4
            }
        }
        18 => 12,
        19 => 13,
        20 => 14,
        21 => 15,
        22 => {
            if room_id == 0x30 {
                16
            } else {
                17
            }
        }
        23 => 17,
        24 => 18,
        25 => 19,
        26 => 20,
        27 => 21,
        29 => 22,
        30..=32 => 24,
        34..=36 => 27,
        37 => 28,
        39 => 29,
        40 => 30,
        41 => 31,
        43 => 33,
        44 => 34,
        45 => 35,
        48..=51 => 36,
        54 => 5,
        58 => 10,
        _ => 0,
    }
}
