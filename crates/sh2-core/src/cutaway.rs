//! Pre-rendered Cutaway Still & Interactive Puzzle Overlay Subsystem (`EvSubPictureDisplay` / `prog_overlay.c`).
//!
//! Renders 2D cutaway stills (`data/pic` or `sh2e/pic`) and generates animated/rotatable overlay
//! sprite quads in the PC's `-256..256` sprite coordinate space using the authentic binary tables
//! extracted from `sh2pc.exe` (Director's Cut).

use crate::events::{EventFlags, PuzzleState, PZ_CLOCK, PZ_WHEEL, PZ_WHEEL_GOAL};

/// Clock hand UV table (`0x8e5c10` in `sh2pc.exe`, 64 bytes).
pub const OV_CLOCK_UV: [u8; 64] = [
    0, 0, 0, 0, 240, 1, 240, 11, 0, 2, 0, 0, 240, 3, 240, 9, 0, 5, 0, 0, 240, 6, 240, 7, 0, 5, 0,
    8, 240, 6, 240, 9, 0, 7, 0, 0, 240, 8, 240, 11, 0, 9, 0, 0, 240, 10, 240, 9, 0, 12, 0, 0, 240,
    13, 240, 7, 0, 12, 0, 8, 240, 13, 240, 9,
];

/// Apartment Room 105 desk coin placement and UV table (`0x8e3208` in `sh2pc.exe`, 68 bytes).
pub const OV_COIN: [u8; 68] = [
    169, 243, 38, 253, 5, 249, 65, 253, 102, 254, 101, 253, 204, 3, 126, 253, 54, 9, 159, 253,
    161, 243, 71, 253, 254, 248, 100, 253, 92, 254, 127, 253, 200, 3, 154, 253, 48, 9, 187, 253,
    159, 243, 39, 253, 252, 248, 64, 253, 96, 254, 97, 253, 196, 3, 132, 253, 41, 9, 158, 253, 0,
    64, 0, 0, 0, 128, 48, 64,
];

/// Hotel briefcase drum alphabet table (`0x88f6bc` in `sh2pc.exe`, 19 bytes).
pub const OV_DRUM_ABC: [u8; 19] = [
    97, 99, 100, 101, 104, 105, 107, 108, 109, 110, 111, 112, 114, 115, 116, 117, 118, 119, 120,
];

/// Hotel briefcase drum letter placement table (`0x8cd270` in `sh2pc.exe`, 912 bytes).
pub const OV_DRUM_POS: [u8; 912] = [
    28, 252, 213, 253, 28, 252, 213, 254, 28, 252, 213, 0, 46, 254, 213, 253, 37, 254, 213, 254,
    29, 254, 219, 0, 45, 0, 213, 253, 45, 0, 213, 254, 45, 0, 213, 0, 57, 2, 213, 253, 49, 2, 222,
    254, 53, 2, 226, 0, 28, 252, 213, 253, 28, 252, 213, 254, 28, 252, 213, 0, 46, 254, 213, 253,
    46, 254, 213, 254, 46, 254, 213, 0, 45, 0, 205, 253, 46, 0, 200, 254, 27, 0, 219, 0, 57, 2,
    213, 253, 57, 2, 213, 254, 57, 2, 213, 0, 26, 252, 197, 253, 4, 252, 201, 254, 18, 252, 215, 0,
    46, 254, 213, 253, 46, 254, 213, 254, 46, 254, 213, 0, 45, 0, 213, 253, 45, 0, 213, 254, 45,
    0, 213, 0, 57, 2, 213, 253, 57, 2, 213, 254, 57, 2, 213, 0, 28, 252, 213, 253, 28, 252, 213,
    254, 28, 252, 213, 0, 46, 254, 213, 253, 60, 254, 213, 254, 46, 254, 213, 0, 45, 0, 213, 253,
    48, 0, 213, 254, 45, 0, 227, 0, 57, 2, 213, 253, 57, 2, 213, 254, 57, 2, 213, 0, 28, 252, 213,
    253, 28, 252, 213, 254, 28, 252, 223, 0, 46, 254, 213, 253, 46, 254, 213, 254, 46, 254, 213,
    0, 45, 0, 213, 253, 45, 0, 213, 254, 45, 0, 213, 0, 57, 2, 213, 253, 57, 2, 213, 254, 57, 2,
    213, 0, 28, 252, 213, 253, 28, 252, 213, 254, 28, 252, 213, 0, 46, 254, 198, 253, 46, 254,
    213, 254, 44, 254, 213, 0, 45, 0, 213, 253, 45, 0, 213, 254, 45, 0, 213, 0, 57, 2, 213, 253,
    57, 2, 213, 254, 57, 2, 213, 0, 28, 252, 213, 253, 24, 252, 205, 254, 28, 252, 213, 0, 46, 254,
    213, 253, 46, 254, 213, 254, 46, 254, 213, 0, 45, 0, 213, 253, 45, 0, 213, 254, 45, 0, 213, 0,
    57, 2, 213, 253, 57, 2, 213, 254, 57, 2, 213, 0, 38, 252, 203, 253, 35, 252, 201, 254, 28, 252,
    213, 0, 46, 254, 213, 253, 46, 254, 213, 254, 46, 254, 213, 0, 57, 0, 205, 253, 66, 0, 207,
    254, 45, 0, 213, 0, 74, 2, 209, 253, 63, 2, 211, 254, 55, 2, 224, 0, 25, 252, 197, 253, 2, 252,
    205, 254, 251, 251, 200, 0, 46, 254, 213, 253, 46, 254, 213, 254, 46, 254, 213, 0, 45, 0, 195,
    253, 22, 0, 207, 254, 30, 0, 213, 0, 57, 2, 213, 253, 57, 2, 213, 254, 57, 2, 213, 0, 28, 252,
    213, 253, 28, 252, 205, 254, 20, 252, 213, 0, 46, 254, 213, 253, 46, 254, 213, 254, 46, 254,
    213, 0, 45, 0, 213, 253, 45, 0, 213, 254, 45, 0, 213, 0, 57, 2, 213, 253, 49, 2, 216, 254, 37,
    2, 223, 0, 28, 252, 213, 253, 16, 252, 205, 254, 28, 252, 213, 0, 40, 254, 209, 253, 36, 254,
    201, 254, 46, 254, 213, 0, 45, 0, 213, 253, 45, 0, 213, 254, 45, 0, 213, 0, 57, 2, 213, 253,
    57, 2, 213, 254, 57, 2, 213, 0, 28, 252, 213, 253, 28, 252, 213, 254, 16, 252, 213, 0, 46, 254,
    213, 253, 56, 254, 217, 254, 45, 254, 233, 0, 45, 0, 213, 253, 45, 0, 213, 254, 45, 0, 213, 0,
    57, 2, 213, 253, 58, 2, 222, 254, 52, 2, 238, 0, 28, 252, 213, 253, 28, 252, 213, 254, 28, 252,
    213, 0, 46, 254, 213, 253, 46, 254, 213, 254, 46, 254, 213, 0, 45, 0, 213, 253, 46, 0, 220,
    254, 43, 0, 229, 0, 57, 2, 213, 253, 51, 2, 223, 254, 57, 2, 213, 0, 24, 252, 213, 253, 24,
    252, 213, 254, 24, 252, 213, 0, 46, 254, 213, 253, 46, 254, 213, 254, 46, 254, 213, 0, 45, 0,
    213, 253, 39, 0, 223, 254, 45, 0, 218, 0, 57, 2, 213, 253, 57, 2, 213, 254, 57, 2, 213, 0, 24,
    252, 213, 253, 24, 252, 213, 254, 24, 252, 226, 0, 46, 254, 213, 253, 46, 254, 213, 254, 46,
    254, 213, 0, 45, 0, 213, 253, 45, 0, 213, 254, 45, 0, 213, 0, 57, 2, 213, 253, 49, 2, 213, 254,
    49, 2, 222, 0, 28, 252, 213, 253, 28, 252, 213, 254, 28, 252, 213, 0, 46, 254, 213, 253, 46,
    254, 213, 254, 46, 254, 213, 0, 45, 0, 213, 253, 45, 0, 213, 254, 45, 0, 213, 0, 57, 2, 213,
    253, 57, 2, 213, 254, 57, 2, 213, 0, 28, 252, 213, 253, 28, 252, 213, 254, 28, 252, 213, 0, 46,
    254, 213, 253, 46, 254, 201, 254, 46, 254, 213, 0, 45, 0, 206, 253, 46, 0, 201, 254, 23, 0,
    221, 0, 57, 2, 213, 253, 57, 2, 213, 254, 57, 2, 213, 0, 28, 252, 213, 253, 28, 252, 213, 254,
    28, 252, 213, 0, 46, 254, 213, 253, 46, 254, 213, 254, 46, 254, 213, 0, 45, 0, 213, 253, 25, 0,
    213, 254, 25, 0, 225, 0, 57, 2, 213, 253, 57, 2, 213, 254, 57, 2, 213, 0, 28, 252, 213, 253,
    28, 252, 213, 254, 28, 252, 213, 0, 46, 254, 213, 253, 46, 254, 225, 254, 46, 254, 223, 0, 45,
    0, 213, 253, 45, 0, 213, 254, 45, 0, 213, 0, 57, 2, 213, 253, 57, 2, 213, 254, 57, 2, 213, 0,
];

/// Baldwin Mansion garden tombstone board UV & placement table (`0x8c7320` in `sh2pc.exe`, 48 bytes).
pub const OV_TEIEN: [u8; 48] = [
    0, 10, 0, 0, 240, 19, 240, 12, 0, 0, 0, 0, 240, 9, 240, 12, 0, 0, 240, 12, 240, 9, 240, 25, 0,
    10, 240, 12, 240, 11, 240, 17, 251, 250, 194, 247, 251, 4, 194, 4, 47, 255, 129, 4, 47, 1, 129,
    9,
];

/// Hospital 2F patient room box locks & symbol wheels table (`0x8db678` in `sh2pc.exe`, 326 bytes).
pub const OV_BOX: [u8; 326] = [
    96, 0, 160, 48, 96, 48, 160, 96, 96, 96, 160, 144, 96, 144, 160, 192, 160, 0, 224, 48, 160, 48,
    224, 96, 160, 96, 224, 144, 160, 144, 224, 192, 0, 160, 64, 208, 0, 0, 0, 0, 177, 246, 73, 1,
    108, 246, 160, 2, 51, 246, 226, 3, 241, 245, 57, 5, 196, 246, 81, 1, 120, 246, 158, 2, 59, 246,
    230, 3, 252, 245, 62, 5, 171, 246, 79, 1, 100, 246, 158, 2, 46, 246, 243, 3, 238, 245, 54, 5,
    178, 246, 74, 1, 99, 246, 155, 2, 47, 246, 227, 3, 239, 245, 48, 5, 202, 246, 74, 1, 127, 246,
    149, 2, 68, 246, 216, 3, 255, 245, 48, 5, 178, 246, 54, 1, 109, 246, 129, 2, 48, 246, 206, 3,
    236, 245, 36, 5, 187, 246, 43, 1, 117, 246, 118, 2, 54, 246, 189, 3, 240, 245, 18, 5, 183, 246,
    103, 1, 118, 246, 176, 2, 52, 246, 254, 3, 239, 245, 80, 5, 183, 246, 47, 1, 118, 246, 114, 2,
    48, 246, 192, 3, 239, 245, 16, 5, 96, 1, 0, 0, 128, 1, 96, 0, 128, 1, 0, 0, 208, 1, 224, 0, 0,
    0, 208, 0, 224, 0, 96, 1, 208, 1, 0, 0, 0, 2, 64, 1, 224, 0, 176, 0, 0, 1, 16, 1, 0, 0, 96, 1,
    16, 1, 192, 1, 0, 0, 192, 1, 96, 0, 0, 2, 96, 0, 224, 1, 160, 0, 0, 2, 16, 1, 224, 0, 176, 1,
    0, 2, 176, 1, 64, 1, 240, 1, 128, 1, 23, 246, 76, 255, 0, 0, 148, 253, 47, 245, 0, 0, 9, 248,
    11, 3, 0, 0, 104, 0, 179, 243, 0, 0, 165, 245, 171, 251, 0, 0, 255, 239, 80, 244, 0, 0, 166,
    247, 115, 0, 0, 0, 68, 253, 132, 255, 0, 0, 226, 2, 136, 244, 0, 0, 117, 12, 43, 251, 0, 0,
    136, 28,
];

/// Hospital wall symbol wheel goal table (`0x8db448` in `sh2pc.exe`, 432 bytes).
pub const OV_WALL: [u8; 432] = [
    0, 0, 0, 0, 32, 0, 64, 0, 160, 0, 0, 0, 192, 0, 64, 0, 64, 1, 0, 0, 96, 1, 64, 0, 0, 0, 128, 0,
    16, 0, 176, 0, 32, 0, 0, 0, 64, 0, 64, 0, 192, 0, 0, 0, 224, 0, 48, 0, 96, 1, 0, 0, 144, 1, 64,
    0, 16, 0, 128, 0, 48, 0, 176, 0, 64, 0, 0, 0, 96, 0, 64, 0, 224, 0, 0, 0, 0, 1, 48, 0, 144, 1,
    0, 0, 176, 1, 64, 0, 48, 0, 128, 0, 80, 0, 192, 0, 96, 0, 0, 0, 128, 0, 64, 0, 0, 1, 0, 0, 32,
    1, 48, 0, 176, 1, 0, 0, 208, 1, 80, 0, 80, 0, 128, 0, 112, 0, 192, 0, 128, 0, 0, 0, 160, 0, 64,
    0, 32, 1, 0, 0, 64, 1, 64, 0, 208, 1, 0, 0, 240, 1, 64, 0, 112, 0, 128, 0, 144, 0, 192, 0, 0,
    0, 64, 0, 32, 0, 128, 0, 160, 0, 64, 0, 192, 0, 128, 0, 64, 1, 64, 0, 112, 1, 112, 0, 144, 0,
    128, 0, 176, 0, 192, 0, 32, 0, 64, 0, 64, 0, 128, 0, 192, 0, 48, 0, 240, 0, 112, 0, 112, 1, 64,
    0, 144, 1, 112, 0, 176, 0, 128, 0, 208, 0, 224, 0, 64, 0, 64, 0, 96, 0, 128, 0, 240, 0, 48, 0,
    16, 1, 112, 0, 144, 1, 80, 0, 192, 1, 144, 0, 208, 0, 128, 0, 240, 0, 192, 0, 96, 0, 64, 0,
    128, 0, 128, 0, 16, 1, 64, 0, 48, 1, 128, 0, 208, 1, 64, 0, 240, 1, 128, 0, 240, 0, 128, 0, 16,
    1, 192, 0, 0, 253, 160, 251, 128, 254, 192, 252, 64, 0, 128, 251, 144, 2, 160, 253, 128, 252,
    208, 251, 128, 254, 32, 253, 0, 0, 112, 251, 65, 2, 32, 253, 128, 252, 224, 251, 80, 254, 96,
    253, 80, 0, 192, 251, 48, 2, 240, 252, 144, 252, 192, 251, 144, 254, 96, 253, 112, 0, 80, 251,
    48, 2, 32, 253, 224, 252, 112, 251, 32, 254, 80, 252, 0, 0, 208, 251, 64, 2, 240, 252, 224,
    252, 96, 251, 32, 254, 144, 252, 0, 0, 160, 252, 32, 2, 144, 253, 0, 253, 128, 251, 48, 254,
    96, 252, 128, 0, 96, 252, 97, 2, 224, 252, 160, 252, 160, 251, 96, 254, 80, 252, 240, 255, 64,
    251, 48, 2, 145, 252, 208, 252, 64, 251, 96, 254, 48, 252, 80, 0, 144, 251, 16, 2, 48, 253,
];

/// Hotel Room 312 VCR placement table (`0x8ceac4` in `sh2pc.exe`, 16 bytes).
pub const OV_VIDEO: [u8; 16] = [
    70, 251, 255, 255, 64, 253, 255, 255, 194, 250, 255, 255, 128, 253, 255, 255,
];

/// Hotel Room 202 briefcase blood-marker code table (`0x8cd600` in `sh2pc.exe`, 466 bytes).
pub const OV_MARKER: [u8; 466] = [
    44, 253, 66, 253, 92, 254, 186, 253, 119, 255, 47, 254, 159, 0, 155, 254, 15, 253, 64, 253, 49,
    254, 169, 253, 92, 255, 38, 254, 113, 0, 165, 254, 224, 252, 80, 253, 11, 254, 198, 253, 57,
    255, 69, 254, 86, 0, 172, 254, 35, 253, 72, 253, 77, 254, 190, 253, 120, 255, 46, 254, 140, 0,
    162, 254, 10, 253, 72, 253, 42, 254, 196, 253, 79, 255, 66, 254, 112, 0, 180, 254, 222, 252,
    77, 253, 4, 254, 193, 253, 38, 255, 56, 254, 70, 0, 180, 254, 249, 252, 72, 253, 33, 254, 184,
    253, 68, 255, 56, 254, 110, 0, 171, 254, 57, 253, 96, 253, 82, 254, 210, 253, 120, 255, 82,
    254, 147, 0, 199, 254, 14, 253, 50, 253, 52, 254, 164, 253, 83, 255, 34, 254, 121, 0, 157, 254,
    17, 253, 64, 253, 56, 254, 184, 253, 89, 255, 58, 254, 124, 0, 175, 254, 253, 252, 92, 253, 30,
    254, 208, 253, 63, 255, 64, 254, 97, 0, 174, 254, 216, 252, 78, 253, 254, 253, 200, 253, 42,
    255, 60, 254, 62, 0, 184, 254, 241, 252, 64, 253, 22, 254, 182, 253, 53, 255, 56, 254, 87, 0,
    165, 254, 223, 252, 64, 253, 4, 254, 183, 253, 26, 255, 46, 254, 75, 0, 159, 254, 236, 252, 64,
    253, 12, 254, 185, 253, 50, 255, 46, 254, 76, 0, 153, 254, 217, 252, 68, 253, 5, 254, 177, 253,
    34, 255, 56, 254, 59, 0, 162, 254, 51, 253, 80, 253, 95, 254, 194, 253, 121, 255, 58, 254, 146,
    0, 181, 254, 235, 252, 62, 253, 13, 254, 178, 253, 48, 255, 50, 254, 84, 0, 165, 254, 22, 253,
    70, 253, 64, 254, 184, 253, 98, 255, 54, 254, 134, 0, 169, 254, 80, 1, 16, 0, 128, 1, 16, 0,
    176, 1, 16, 0, 80, 1, 64, 0, 128, 1, 64, 0, 176, 1, 64, 0, 80, 1, 112, 0, 128, 1, 112, 0, 176,
    1, 112, 0, 112, 1, 160, 0, 160, 1, 160, 0, 208, 1, 160, 0, 0, 0, 32, 1, 48, 0, 32, 1, 96, 0,
    32, 1, 0, 0, 80, 1, 64, 0, 80, 1, 0, 0, 128, 1, 64, 0, 128, 1, 123, 244, 137, 246, 123, 235,
    30, 232, 0, 0, 0, 0, 0, 0, 0, 0, 159, 0, 31, 1, 160, 0, 0, 0, 191, 0, 255, 0, 192, 0, 0, 0, 79,
    1, 223, 0, 80, 1, 160, 0, 111, 1, 223, 0, 48, 1, 224, 0, 79, 1, 255, 0, 144, 0, 32, 1, 255, 1,
    255, 1, 160, 0, 0, 1, 79, 1, 31, 1, 192, 0, 224, 0, 47, 1, 255, 0, 80, 1, 224, 0, 255, 1, 31,
    1, 33, 78,
];

/// Hospital elevator button table (`0x8da828` in `sh2pc.exe`, 96 bytes).
pub const OV_ELEVATOR: [u8; 96] = [
    0, 0, 0, 0, 160, 0, 0, 0, 64, 0, 0, 0, 240, 0, 0, 0, 0, 0, 0, 0, 80, 0, 0, 0, 64, 0, 0, 0, 160,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 64, 0, 0, 0, 80, 0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0, 128, 0, 0,
    0, 80, 0, 0, 0, 160, 1, 0, 0, 112, 245, 255, 255, 144, 1, 0, 0, 208, 248, 255, 255, 144, 1, 0,
    0, 48, 252, 255, 255, 208, 255, 255, 255, 176, 5, 0, 0,
];

/// Otherworld Hospital elevator button table (`0x8d83d0` in `sh2pc.exe`, 96 bytes).
pub const OV_ELEVATOR2: [u8; 96] = [
    0, 0, 0, 0, 160, 0, 0, 0, 64, 0, 0, 0, 240, 0, 0, 0, 0, 0, 0, 0, 80, 0, 0, 0, 64, 0, 0, 0, 160,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 64, 0, 0, 0, 80, 0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0, 128, 0, 0,
    0, 80, 0, 0, 0, 160, 1, 0, 0, 112, 245, 255, 255, 144, 1, 0, 0, 208, 248, 255, 255, 144, 1, 0,
    0, 48, 252, 255, 255, 208, 255, 255, 255, 176, 5, 0, 0,
];

/// Original PS2/PC dimensions `(width, height)` for all 17 overlay sprite sheets (`pics.py` `SHEETS`).
pub const SHEET_ORIGINAL_DIMENSIONS: [(&str, (u32, u32)); 17] = [
    ("apt/clock_hari", (256, 256)),
    ("apt/p_safe_close2", (128, 128)),
    ("apt/p_desk_coin_coin", (64, 256)),
    ("hsp/pboxkey01", (512, 512)),
    ("hsp/p_boxnumber_2", (512, 256)),
    ("hsp/p_quiz_btn", (128, 128)),
    ("htl/dram", (256, 256)),
    ("htl/egg", (64, 64)),
    ("add/teien2", (512, 512)),
    ("htl/maker", (512, 512)),
    ("htl/p_video02", (256, 128)),
    ("hsp/p_h_elevator_botan", (128, 256)),
    ("dls/p_manacles_off", (512, 512)),
    ("hsp/p_hair_hair", (256, 64)),
    ("hsp/p_female_ring", (64, 64)),
    ("dls/p_tablet_plate", (256, 256)),
    ("out/p_incar_map", (256, 256)),
];

/// Looks up the original PS2/PC texel dimensions `(width, height)` for an overlay sprite sheet.
pub fn original_sheet_dimensions(sheet_name: &str) -> (u32, u32) {
    SHEET_ORIGINAL_DIMENSIONS
        .iter()
        .find(|&&(name, _)| name == sheet_name)
        .map(|&(_, dims)| dims)
        .unwrap_or((256, 256))
}

/// Interactive puzzle or cutaway still overlay kind (`prog_overlay.c`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayKind {
    /// No sprite sheet overlay over the active still.
    None,
    /// Room 208 closed clock face (`apt_clock_close` + `apt/clock_hari`).
    ClockClosed,
    /// Room 208 open clock face (`apt_clock_open` + `apt/clock_hari`).
    ClockOpen,
    /// Room 203 / W2F safe dial (`apt_p_safe_close` + `apt/p_safe_close2`).
    Safe,
    /// Room 105 five-slot coin desk (`apt_p_desk_coin` + `apt/p_desk_coin_coin`).
    Coins,
    /// Hospital 2F locked box (`hsp_p_box` + `hsp/pboxkey01`).
    Box,
    /// Hospital 3F wall symbol blood scrawl (`hsp_p_boxnumber` + `hsp/p_boxnumber_2`).
    BoxWall,
    /// Hospital Trick or Treat quiz box (`hsp_p_quiz` + `hsp/p_quiz_btn`).
    Quiz,
    /// Hotel cloakroom briefcase letter drums (`htl_p_trank` + `htl/dram`).
    Drums,
    /// Hotel 3F doors with Scarlet / Rust-colored eggs (`htl_p_egghollow` + `htl/egg`).
    Egg,
    /// Baldwin Mansion garden tombstone board puzzle (`add_p_teien` + `add/teien2`).
    Tombstone,
    /// Hotel Room 312 VCR (`htl_p_video` + `htl/p_video02`).
    Video,
    /// Hotel Room 202 open briefcase with marker (`htl_p_marker` + `htl/maker`).
    Marker,
    /// Hospital elevator button panel (`hsp_p_h_elevator` + `hsp/p_h_elevator_botan`).
    Elevator,
    /// Otherworld Hospital elevator button panel (`hsp_p_h_elevator_2` + `hsp/p_h_elevator_botan`).
    Elevator2,
    /// Labyrinth catacombs manacles (`dls_p_manacles_on` + `dls/p_manacles_off`).
    Manacles,
    /// Observation deck car seat Silent Hill map (`out_p_incar` + `out/p_incar_map`).
    InCar,
    /// Memo examine overlay (`0x12`, `0x20`, `0x21`, `0x23`, `0x29`, `0x35`, `0x36`).
    Memo(u8),
}

/// Single textured sprite quad emitted by `CutawayOverlayEngine` over the active cutaway still.
#[derive(Debug, Clone, PartialEq)]
pub struct OverlaySpriteQuad {
    /// Sprite sheet path under `pic/` (e.g. `"out/p_incar_map"`, `"apt/clock_hari"`).
    pub sheet_name: &'static str,
    /// Four corners `(x, y, u, v)` in PC screen space (`x: -256..256`, `y: -256..256`) and
    /// normalized texture coordinates (`u, v: 0.0..=1.0`).
    pub vertices: [(f32, f32, f32, f32); 4],
    /// Opacity in `0.0..=1.0`.
    pub alpha: f32,
}

impl OverlaySpriteQuad {
    /// Converts the four PC sprite-space vertices (`-256..256`) into a 6-vertex triangle pair
    /// `(x, y, u, v)` inside a target viewport rectangle `(vp_x, vp_y, vp_w, vp_h)`.
    pub fn to_viewport_triangles(
        &self,
        vp_x: f32,
        vp_y: f32,
        vp_w: f32,
        vp_h: f32,
    ) -> [(f32, f32, f32, f32); 6] {
        let map_pt = |(px, py, u, v): (f32, f32, f32, f32)| -> (f32, f32, f32, f32) {
            let sx = vp_x + (px + 256.0) * (vp_w / 512.0);
            let sy = vp_y + (py + 256.0) * (vp_h / 512.0);
            (sx, sy, u, v)
        };
        let p0 = map_pt(self.vertices[0]);
        let p1 = map_pt(self.vertices[1]);
        let p2 = map_pt(self.vertices[2]);
        let p3 = map_pt(self.vertices[3]);
        [p0, p1, p2, p0, p2, p3]
    }
}

fn read_i16(buf: &[u8], byte_offset: usize) -> i16 {
    i16::from_le_bytes([buf[byte_offset], buf[byte_offset + 1]])
}

fn read_i32(buf: &[u8], byte_offset: usize) -> i32 {
    i32::from_le_bytes([
        buf[byte_offset],
        buf[byte_offset + 1],
        buf[byte_offset + 2],
        buf[byte_offset + 3],
    ])
}

/// Stateful cutaway still & interactive puzzle sprite overlay engine (`prog_overlay.c`).
#[derive(Debug, Clone)]
pub struct CutawayOverlayEngine {
    /// Active still picture identifier (e.g. `"out_p_incar"`, `"apt_clock_open"`), if any.
    active_picture: Option<String>,
    /// Active overlay kind derived from `active_picture`.
    kind: OverlayKind,
    /// Active sprite sheet name (`"out/p_incar_map"`, `"apt/clock_hari"`, etc.), if any.
    sheet_name: Option<&'static str>,
    /// Alpha fade for `dls_p_manacles_on` (`0.0..=1.0`).
    manacles_alpha: f32,
    /// Alpha fade for `out_p_incar` car seat map (`1.0` -> `0.0` over 1.0s when taken).
    incar_alpha: f32,
    /// Sprite quads generated for the current frame.
    frame_sprites: Vec<OverlaySpriteQuad>,
}

impl Default for CutawayOverlayEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl CutawayOverlayEngine {
    /// Creates an idle cutaway overlay engine.
    pub fn new() -> Self {
        Self {
            active_picture: None,
            kind: OverlayKind::None,
            sheet_name: None,
            manacles_alpha: 0.0,
            incar_alpha: 1.0,
            frame_sprites: Vec::new(),
        }
    }

    /// Returns the currently displayed base cutaway still identifier, if any.
    pub fn active_picture(&self) -> Option<&str> {
        self.active_picture.as_deref()
    }

    /// Returns the active overlay kind.
    pub fn kind(&self) -> OverlayKind {
        self.kind
    }

    /// Returns the active sprite sheet identifier, if any.
    pub fn sheet_name(&self) -> Option<&'static str> {
        self.sheet_name
    }

    /// Returns the sprite quads generated on the most recent frame.
    pub fn frame_sprites(&self) -> &[OverlaySpriteQuad] {
        &self.frame_sprites
    }

    /// Sets the alpha of the open manacles overlay (`0x57fd50`).
    pub fn set_manacles_alpha(&mut self, alpha: f32) {
        self.manacles_alpha = alpha.clamp(0.0, 1.0);
    }

    /// Sets the alpha of the car seat map overlay (`0x5a0b80`).
    pub fn set_incar_alpha(&mut self, alpha: f32) {
        self.incar_alpha = alpha.clamp(0.0, 1.0);
    }

    /// Returns the current car seat map overlay alpha (`0.0..=1.0`).
    pub fn incar_alpha(&self) -> f32 {
        self.incar_alpha
    }

    /// Returns the current open manacles overlay alpha (`0.0..=1.0`).
    pub fn manacles_alpha(&self) -> f32 {
        self.manacles_alpha
    }

    /// Advances active overlay alpha transitions (`out_p_incar` map fade and `dls_p_manacles_on` fade).
    pub fn tick(&mut self, dt: f32, flags: &EventFlags) {
        if dt <= 0.0 {
            return;
        }
        match self.kind {
            OverlayKind::InCar => {
                if flags.get(28) {
                    self.incar_alpha = (self.incar_alpha - dt).max(0.0);
                } else {
                    self.incar_alpha = 1.0;
                }
            }
            OverlayKind::Manacles => {
                if flags.get(277) || flags.get(304) {
                    self.manacles_alpha = (self.manacles_alpha + dt).min(1.0);
                } else {
                    self.manacles_alpha = 0.0;
                }
            }
            _ => {}
        }
    }

    /// Activates a cutaway still (`prog_overlay_picture`), automatically binding the matching
    /// interactive puzzle sprite sheet (`SHEETS`) when applicable. Pass `None` to hide the still.
    pub fn set_picture(&mut self, pic: Option<&str>, flags: &EventFlags) {
        self.frame_sprites.clear();
        let Some(pic_str) = pic.filter(|s| !s.is_empty()) else {
            self.active_picture = None;
            self.kind = OverlayKind::None;
            self.sheet_name = None;
            return;
        };

        self.active_picture = Some(pic_str.to_string());
        let (kind, sheet) = match pic_str {
            "apt_clock_close" => (OverlayKind::ClockClosed, Some("apt/clock_hari")),
            "apt_clock_open" => (OverlayKind::ClockOpen, Some("apt/clock_hari")),
            "apt_p_safe_close" => (OverlayKind::Safe, Some("apt/p_safe_close2")),
            "apt_p_desk_coin" => (OverlayKind::Coins, Some("apt/p_desk_coin_coin")),
            "hsp_p_box" => (OverlayKind::Box, Some("hsp/pboxkey01")),
            "hsp_p_boxnumber" => (OverlayKind::BoxWall, Some("hsp/p_boxnumber_2")),
            "hsp_p_quiz" => (OverlayKind::Quiz, Some("hsp/p_quiz_btn")),
            "htl_p_trank" => (OverlayKind::Drums, Some("htl/dram")),
            "htl_p_egghollow" => (OverlayKind::Egg, Some("htl/egg")),
            "add_p_teien" => (OverlayKind::Tombstone, Some("add/teien2")),
            "htl_p_video" => (OverlayKind::Video, Some("htl/p_video02")),
            "htl_p_marker" => (OverlayKind::Marker, Some("htl/maker")),
            "hsp_p_h_elevator" => (OverlayKind::Elevator, Some("hsp/p_h_elevator_botan")),
            "hsp_p_h_elevator_2" => (OverlayKind::Elevator2, Some("hsp/p_h_elevator_botan")),
            "dls_p_manacles_on" => {
                self.manacles_alpha = if flags.get(277) { 1.0 } else { 0.0 };
                (OverlayKind::Manacles, Some("dls/p_manacles_off"))
            }
            "out_p_incar" => {
                self.incar_alpha = if flags.get(28) { 0.0 } else { 1.0 };
                (OverlayKind::InCar, Some("out/p_incar_map"))
            }
            _ => (OverlayKind::None, None),
        };
        self.kind = kind;
        self.sheet_name = sheet;
    }

    /// Evaluates and emits the sprite quads for the current frame (`prog_overlay_frame`).
    ///
    /// `extra` carries context-specific state (e.g. the memo index for `OverlayKind::Memo`, or the
    /// current elevator floor bitmask `1..=8` for `OverlayKind::Elevator` / `Elevator2`).
    pub fn evaluate_frame(
        &mut self,
        flags: &EventFlags,
        puzzle: &PuzzleState,
        riddle_difficulty: u8,
        extra: i32,
    ) -> &[OverlaySpriteQuad] {
        self.frame_sprites.clear();

        if self.kind == OverlayKind::None {
            // Memo overlays when viewing specific stills in the memo screen (0x4997c0..0x499d00)
            match extra {
                0x12 => {
                    if !flags.get(120) {
                        self.emit_rect(
                            "apt/p_dust_out_coin",
                            24.0,
                            37.0,
                            56.0,
                            69.0,
                            0.0,
                            0.0,
                            32.0,
                            32.0,
                            1.0,
                        );
                    }
                }
                0x20 => {
                    if !flags.get(186) {
                        self.emit_rect(
                            "hsp/p_hair_hair",
                            -89.0,
                            12.0,
                            57.0,
                            50.0,
                            0.0,
                            0.0,
                            146.0,
                            38.0,
                            1.0,
                        );
                    }
                }
                0x21 => {
                    if !flags.get(187) {
                        self.emit_rect(
                            "hsp/p_female_ring",
                            -21.0,
                            -65.0,
                            5.0,
                            -17.0,
                            0.0,
                            0.0,
                            26.0,
                            48.0,
                            1.0,
                        );
                    }
                }
                0x23 => {
                    for i in 0..3usize {
                        if flags.get(278 + i) {
                            let x = [-31.0f32, -89.0, 29.0][i];
                            let u = [0.0f32, 64.0, 128.0][i];
                            let v = [0.0f32, 0.0, 128.0][i];
                            self.emit_rect(
                                "dls/p_tablet_plate",
                                x,
                                56.0,
                                x + 60.0,
                                122.0,
                                u,
                                v,
                                u + 60.0,
                                v + 66.0,
                                1.0,
                            );
                        }
                    }
                }
                0x29 => {
                    for i in 0..3usize {
                        if flags.get(382 + i) {
                            let x = [-225.0f32, -45.0, 134.0][i];
                            self.emit_rect(
                                "htl/egg",
                                x,
                                -123.0,
                                x + 48.0,
                                -68.0,
                                0.0,
                                0.0,
                                48.0,
                                55.0,
                                1.0,
                            );
                        }
                    }
                }
                0x35 => {
                    let prev_sheet = self.sheet_name;
                    self.sheet_name = Some("htl/p_video02");
                    self.draw_video(flags);
                    self.sheet_name = prev_sheet;
                }
                0x36 => {
                    let prev_sheet = self.sheet_name;
                    self.sheet_name = Some("htl/maker");
                    self.draw_marker(flags, riddle_difficulty);
                    self.sheet_name = prev_sheet;
                }
                _ => {}
            }
            return &self.frame_sprites;
        }

        match self.kind {
            OverlayKind::ClockClosed => self.draw_clock(puzzle, false),
            OverlayKind::ClockOpen => self.draw_clock(puzzle, true),
            OverlayKind::Safe => self.draw_safe(flags),
            OverlayKind::Coins => self.draw_coins(flags),
            OverlayKind::Box => self.draw_box(flags, puzzle),
            OverlayKind::BoxWall => self.draw_box_wall(puzzle),
            OverlayKind::Quiz => self.draw_quiz(flags),
            OverlayKind::Drums => self.draw_drums(flags, puzzle),
            OverlayKind::Egg => self.draw_egg(flags),
            OverlayKind::Tombstone => self.draw_tombstone(flags),
            OverlayKind::Video => self.draw_video(flags),
            OverlayKind::Marker => self.draw_marker(flags, riddle_difficulty),
            OverlayKind::Elevator => self.draw_elevator(&OV_ELEVATOR, extra),
            OverlayKind::Elevator2 => self.draw_elevator(&OV_ELEVATOR2, extra),
            OverlayKind::Manacles => {
                if self.manacles_alpha > 0.0 {
                    self.emit_rect(
                        "dls/p_manacles_off",
                        -256.0,
                        -256.0,
                        256.0,
                        256.0,
                        0.0,
                        0.0,
                        512.0,
                        512.0,
                        self.manacles_alpha,
                    );
                }
            }
            OverlayKind::InCar => {
                if self.incar_alpha > 0.0 {
                    // 0x5a0b80: (-127, -234) size (256, 256)
                    self.emit_rect(
                        "out/p_incar_map",
                        -127.0,
                        -234.0,
                        129.0,
                        22.0,
                        0.0,
                        0.0,
                        256.0,
                        256.0,
                        self.incar_alpha,
                    );
                }
            }
            OverlayKind::None | OverlayKind::Memo(_) => {}
        }

        &self.frame_sprites
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_rect(
        &mut self,
        sheet_name: &'static str,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        u0_px: f32,
        v0_px: f32,
        u1_px: f32,
        v1_px: f32,
        alpha: f32,
    ) {
        let (sw, sh) = original_sheet_dimensions(sheet_name);
        let sw = sw as f32;
        let sh = sh as f32;
        let u0 = u0_px / sw;
        let v0 = v0_px / sh;
        let u1 = u1_px / sw;
        let v1 = v1_px / sh;
        self.frame_sprites.push(OverlaySpriteQuad {
            sheet_name,
            vertices: [(x0, y0, u0, v0), (x1, y0, u1, v0), (x1, y1, u1, v1), (x0, y1, u0, v1)],
            alpha,
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_quad(
        &mut self,
        sheet_name: &'static str,
        pts: [(f32, f32); 4],
        u0_px: f32,
        v0_px: f32,
        u1_px: f32,
        v1_px: f32,
        alpha: f32,
    ) {
        let (sw, sh) = original_sheet_dimensions(sheet_name);
        let sw = sw as f32;
        let sh = sh as f32;
        let u0 = u0_px / sw;
        let v0 = v0_px / sh;
        let u1 = u1_px / sw;
        let v1 = v1_px / sh;
        self.frame_sprites.push(OverlaySpriteQuad {
            sheet_name,
            vertices: [
                (pts[0].0, pts[0].1, u0, v0),
                (pts[1].0, pts[1].1, u1, v0),
                (pts[2].0, pts[2].1, u1, v1),
                (pts[3].0, pts[3].1, u0, v1),
            ],
            alpha,
        });
    }

    // ---------------------------------------------------------------- clock (0x59a390)
    fn draw_clock(&mut self, puzzle: &PuzzleState, open: bool) {
        let t = puzzle.read_u16(PZ_CLOCK);
        let hour = (t as f32) * (std::f32::consts::TAU / 46080.0);
        let min = ((t % 3840) as f32) * (std::f32::consts::TAU / 3840.0);
        let sec = ((t & 63) as f32) * (std::f32::consts::TAU / 64.0);
        let ang = [min, hour, sec, sec];

        let (cx, cy, s) = if open {
            (-11.0f32, -9.0f32, 1.0f32)
        } else {
            (-6.0f32, -26.0f32, 43.0f32 / 68.0f32)
        };
        let hw = [16.0 * s, 16.0 * s, 12.0 * s, 12.0 * s];
        let ht = [192.0 * s, 160.0 * s, 128.0 * s, 0.0];
        let hb = [0.0, 0.0, 0.0, -32.0 * s];
        let base = if open { 16usize } else { 0usize };

        for i in 0..4usize {
            let cs = ang[i].cos();
            let sn = ang[i].sin();
            let x = [-hw[i], hw[i], hw[i], -hw[i]];
            let y = [-ht[i], -ht[i], -hb[i], -hb[i]];
            let mut pts = [(0.0f32, 0.0f32); 4];
            for k in 0..4 {
                pts[k].0 = (x[k] * cs - y[k] * sn) * 0.8 + cx;
                pts[k].1 = x[k] * sn + y[k] * cs + cy;
            }
            let uv_ofs = (base + i * 4) * 2;
            let u0 = (read_i16(&OV_CLOCK_UV, uv_ofs) >> 4) as f32;
            let v0 = (read_i16(&OV_CLOCK_UV, uv_ofs + 2) >> 4) as f32;
            let u1 = (read_i16(&OV_CLOCK_UV, uv_ofs + 4) >> 4) as f32;
            let v1 = (read_i16(&OV_CLOCK_UV, uv_ofs + 6) >> 4) as f32;
            self.emit_quad("apt/clock_hari", pts, u0, v0, u1, v1, 1.0);
        }
    }

    // ---------------------------------------------------------------- safe (0x5957a0)
    fn draw_safe(&mut self, flags: &EventFlags) {
        let mut pos = 0i32;
        for b in 0..5usize {
            if flags.get(103 + b) {
                pos |= 1 << b;
            }
        }
        let a = (-pos as f32) * (std::f32::consts::TAU / 20.0);
        let cs = a.cos();
        let sn = a.sin();
        let c = [(-64.0f32, -64.0f32), (64.0, -64.0), (64.0, 64.0), (-64.0, 64.0)];
        let mut pts = [(0.0f32, 0.0f32); 4];
        for k in 0..4 {
            pts[k].0 = (c[k].0 * cs - c[k].1 * sn) * 0.8 - 1.0;
            pts[k].1 = c[k].0 * sn + c[k].1 * cs - 2.0;
        }
        self.emit_quad("apt/p_safe_close2", pts, 0.0, 0.0, 128.0, 128.0, 1.0);
    }

    // ---------------------------------------------------------------- coins (0x597ce0)
    fn draw_coins(&mut self, flags: &EventFlags) {
        const EE_UV: [[i16; 4]; 3] = [
            [0, 1055, 1024, 2075],
            [0, 0, 1024, 1024],
            [-4, 2082, 1028, 3108],
        ];
        const EE_XY: [[i16; 4]; 5] = [
            [-3160, -708, -2090, 362],
            [-1790, -682, -720, 388],
            [-414, -650, 656, 420],
            [962, -612, 2032, 458],
            [2348, -580, 3418, 490],
        ];
        let _ = OV_COIN.len();
        for (hole, xy) in EE_XY.iter().enumerate() {
            for (c, uv) in EE_UV.iter().enumerate() {
                if !flags.get(126 + 5 * c + hole) {
                    continue;
                }
                self.emit_rect(
                    "apt/p_desk_coin_coin",
                    (xy[0] as f32) / 16.0,
                    (xy[1] as f32) / 16.0,
                    (xy[2] as f32) / 16.0,
                    (xy[3] as f32) / 16.0,
                    (uv[0] as f32) / 16.0,
                    (uv[1] as f32) / 16.0,
                    (uv[2] as f32) / 16.0,
                    (uv[3] as f32) / 16.0,
                    1.0,
                );
                break;
            }
        }
    }

    // ---------------------------------------------------------------- hospital box & wall (0x58e1f0, 0x58db10)
    fn draw_box(&mut self, flags: &EventFlags, puzzle: &PuzzleState) {
        if !flags.get(195) {
            let r = &OV_BOX[184..];
            let pos = &OV_BOX[264..];
            for pass in 0..2usize {
                let range = if pass != 0 { 3..10 } else { 0..3 };
                for i in range {
                    let x = (read_i16(pos, 6 * i) as f32) / 16.0;
                    let y = (read_i16(pos, 6 * i + 2) as f32) / 16.0;
                    let u0 = read_i16(r, 8 * i) as f32;
                    let v0 = read_i16(r, 8 * i + 2) as f32;
                    let u1 = read_i16(r, 8 * i + 4) as f32;
                    let v1 = read_i16(r, 8 * i + 6) as f32;
                    self.emit_rect(
                        "hsp/pboxkey01",
                        x,
                        y,
                        x + (u1 - u0),
                        y + (v1 - v0),
                        u0,
                        v0,
                        u1 - 1.0,
                        v1 - 1.0,
                        1.0,
                    );
                }
            }
        }

        if !flags.get(197) {
            self.emit_rect(
                "hsp/pboxkey01",
                -163.625,
                -6.6875,
                -83.625,
                153.3125,
                0.0,
                0.0,
                79.0,
                159.0,
                1.0,
            );
            let w = &puzzle.raw()[PZ_WHEEL..PZ_WHEEL + 4];
            for (i, &wheel_val) in w.iter().enumerate() {
                let s = (wheel_val as usize) % 9;
                let r = &OV_BOX[4 * s..4 * s + 4];
                let ofs = 40 + 4 * (i + 4 * s);
                let x = (read_i16(&OV_BOX, ofs) as f32) / 16.0;
                let y = (read_i16(&OV_BOX, ofs + 2) as f32) / 16.0;
                let u0 = r[0] as f32;
                let v0 = r[1] as f32;
                let u1 = r[2] as f32;
                let v1 = r[3] as f32;
                self.emit_rect(
                    "hsp/pboxkey01",
                    x,
                    y,
                    x + (u1 - u0),
                    y + (v1 - v0),
                    u0,
                    v0,
                    u1 - 1.0,
                    v1 - 1.0,
                    1.0,
                );
            }
        }

        if !flags.get(198) {
            self.emit_rect(
                "hsp/pboxkey01",
                24.1875,
                38.75,
                152.1875,
                198.75,
                224.0,
                0.0,
                351.0,
                159.0,
                1.0,
            );
        }
    }

    fn draw_box_wall(&mut self, puzzle: &PuzzleState) {
        let g = &puzzle.raw()[PZ_WHEEL_GOAL..PZ_WHEEL_GOAL + 4];
        for (i, &goal_val) in g.iter().enumerate() {
            let e = i + 4 * ((goal_val as usize) % 9);
            let x = (read_i16(&OV_WALL, 288 + 4 * e) as f32) / 16.0;
            let y = (read_i16(&OV_WALL, 288 + 4 * e + 2) as f32) / 16.0;
            let u0 = read_i16(&OV_WALL, 8 * e) as f32;
            let v0 = read_i16(&OV_WALL, 8 * e + 2) as f32;
            let u1 = read_i16(&OV_WALL, 8 * e + 4) as f32;
            let v1 = read_i16(&OV_WALL, 8 * e + 6) as f32;
            self.emit_rect(
                "hsp/p_boxnumber_2",
                x,
                y,
                x + (u1 - u0),
                y + (v1 - v0),
                u0,
                v0,
                u1 - 1.0,
                v1 - 1.0,
                1.0,
            );
        }
    }

    // ---------------------------------------------------------------- quiz box (0x589820)
    fn draw_quiz(&mut self, flags: &EventFlags) {
        const POS: [[i16; 2]; 9] = [
            [-262, -2033],
            [39, -1936],
            [333, -1839],
            [-422, -1774],
            [-109, -1682],
            [182, -1569],
            [-590, -1469],
            [-298, -1381],
            [32, -1252],
        ];
        for r in 0..3usize {
            let a = flags.get(234 + 2 * r);
            let b = flags.get(235 + 2 * r);
            if !a && !b {
                continue;
            }
            let c = if a && b {
                2usize
            } else if b {
                1
            } else {
                0
            };
            let i = 3 * r + c;
            let x = (POS[i][0] as f32) / 16.0;
            let y = (POS[i][1] as f32) / 16.0;
            let u = 30.0 * (c as f32);
            let v = 30.0 * (r as f32);
            self.emit_rect(
                "hsp/p_quiz_btn",
                x,
                y,
                x + 30.0,
                y + 30.0,
                u,
                v,
                u + 30.0,
                v + 30.0,
                1.0,
            );
        }
    }

    // ---------------------------------------------------------------- hotel trunk drums (0x579af0)
    fn draw_drums(&mut self, flags: &EventFlags, puzzle: &PuzzleState) {
        for d in 0..4usize {
            let pos = if flags.get(405) {
                (puzzle.raw()[0x1c + d] & 7) as i32
            } else {
                ((d as i32) * 3 + 2) & 7
            };
            for r in -1i32..=1 {
                let mut c = b'a' + ((((pos + r) & 7) * 3 + (d as i32)) as u8);
                if c > b'z' {
                    c -= 26;
                }
                let mut a = 0usize;
                while a < 18 && OV_DRUM_ABC[a] != c {
                    a += 1;
                }
                let idx = (a * 12 + d * 3 + ((r + 1) as usize)) * 4;
                let x = read_i16(&OV_DRUM_POS, idx) as f32;
                let y = read_i16(&OV_DRUM_POS, idx + 2) as f32;
                let u = ((a % 6) * 32) as f32;
                let v = ((a / 6) * 64 + ((r + 1) as usize) * 16) as f32;
                let h = if r == 0 { 24.0 } else { 15.0 };
                self.emit_rect("htl/dram", x, y, x + 28.0, y + h, u, v, u + 32.0, v + h, 1.0);
            }
        }
    }

    // ---------------------------------------------------------------- hotel egg doors (0x57bd40)
    fn draw_egg(&mut self, flags: &EventFlags) {
        if flags.get(380) {
            self.emit_rect(
                "htl/egg",
                -157.0,
                -41.0,
                -109.0,
                14.0,
                0.0,
                0.0,
                48.0,
                55.0,
                1.0,
            );
        }
        if flags.get(381) {
            self.emit_rect(
                "htl/egg",
                94.0,
                -40.0,
                142.0,
                15.0,
                0.0,
                0.0,
                48.0,
                55.0,
                1.0,
            );
        }
    }

    // ---------------------------------------------------------------- Baldwin tombstone (0x571f50)
    fn draw_tombstone(&mut self, flags: &EventFlags) {
        let mut pts = [(0.0f32, 0.0f32); 4];
        for (i, pt) in pts.iter_mut().enumerate() {
            let p_ofs = (16 + i * 2) * 2;
            pt.0 = (read_i16(&OV_TEIEN, p_ofs) as f32) * 0.0625;
            pt.1 = (read_i16(&OV_TEIEN, p_ofs + 2) as f32) * 0.0625;
        }
        // Extra flag 704: Acacia key in the relief
        if !flags.get(704) {
            let u0 = (read_i16(&OV_TEIEN, 0) >> 4) as f32;
            let v0 = (read_i16(&OV_TEIEN, 2) >> 4) as f32;
            let u1 = (read_i16(&OV_TEIEN, 4) >> 4) as f32;
            let v1 = (read_i16(&OV_TEIEN, 6) >> 4) as f32;
            self.emit_quad("add/teien2", pts, u0, v0, u1, v1, 1.0);
        }
        // White, Black, Red boards (extra flags 691..693), rotated by flags 694..699 (2 bits each)
        for b in 0..3usize {
            if flags.get(691 + b) {
                let mut rot = if flags.get(694 + b * 2) { 1usize } else { 0usize }
                    | if flags.get(695 + b * 2) { 2usize } else { 0usize };
                if b > 0 && (rot & 1) != 0 {
                    rot ^= 2;
                }
                let mut rpts = [(0.0f32, 0.0f32); 4];
                for k in 0..4 {
                    rpts[k] = pts[(k + rot) & 3];
                }
                let uv_ofs = (1 + b) * 8;
                let u0 = (read_i16(&OV_TEIEN, uv_ofs) >> 4) as f32;
                let v0 = (read_i16(&OV_TEIEN, uv_ofs + 2) >> 4) as f32;
                let u1 = (read_i16(&OV_TEIEN, uv_ofs + 4) >> 4) as f32;
                let v1 = (read_i16(&OV_TEIEN, uv_ofs + 6) >> 4) as f32;
                self.emit_quad("add/teien2", rpts, u0, v0, u1, v1, 1.0);
            }
        }
    }

    // ---------------------------------------------------------------- VCR (0x57b5a0) & briefcase marker (0x57a0d0)
    fn draw_video(&mut self, flags: &EventFlags) {
        if !flags.get(388) {
            return;
        }
        let s = if flags.get(389) { 1usize } else { 0usize };
        let x = read_i32(&OV_VIDEO, s * 8) as f32 * 0.0625;
        let y = read_i32(&OV_VIDEO, s * 8 + 4) as f32 * 0.0625;
        let w = if s != 0 { 160.0 } else { 148.0 };
        let h = if s != 0 { 32.0 } else { 20.0 };
        let v = if s != 0 { 24.0 } else { 0.0 };
        self.emit_rect("htl/p_video02", x, y, x + w, y + h, 0.0, v, w, v + h, 1.0);
    }

    fn draw_marker(&mut self, flags: &EventFlags, riddle_difficulty: u8) {
        let mut word = 0usize;
        for i in 0..5usize {
            if flags.get(406 + i) {
                word |= 1 << i;
            }
        }
        if word >= 19 {
            word = 0;
        }
        for i in 0..4usize {
            let p_ofs = (word * 8 + i * 2) * 2;
            let x = (read_i16(&OV_MARKER, p_ofs) >> 4) as f32;
            let y = (read_i16(&OV_MARKER, p_ofs + 2) >> 4) as f32;
            let u_ofs = (152 + (word / 2) * 2) * 2;
            let u = read_i16(&OV_MARKER, u_ofs) as f32;
            let v = (read_i16(&OV_MARKER, u_ofs + 2) + ((word & 1) as i16) * 24) as f32;
            self.emit_rect("htl/maker", x, y, x + 24.0, y + 24.0, u, v, u + 24.0, v + 24.0, 1.0);
        }
        if !flags.get(404) {
            let x = (read_i16(&OV_MARKER, 188 * 2) >> 4) as f32;
            let y = (read_i16(&OV_MARKER, 189 * 2) >> 4) as f32;
            self.emit_rect(
                "htl/maker",
                x,
                y,
                x + 256.0,
                y + 128.0,
                0.0,
                0.0,
                256.0,
                128.0,
                1.0,
            );
        }
        let r = (riddle_difficulty as usize).min(3);
        let p_ofs = (176 + r * 2) * 2;
        let u_ofs = (206 + r * 4) * 2;
        let x0 = read_i16(&OV_MARKER, p_ofs) as f32;
        let y0 = read_i16(&OV_MARKER, p_ofs + 2) as f32;
        let u0 = read_i16(&OV_MARKER, u_ofs) as f32;
        let v0 = read_i16(&OV_MARKER, u_ofs + 2) as f32;
        let u1 = (read_i16(&OV_MARKER, u_ofs + 4) + 1) as f32;
        let v1 = (read_i16(&OV_MARKER, u_ofs + 6) + 1) as f32;
        self.emit_rect(
            "htl/maker",
            x0,
            y0,
            x0 + (u1 - u0),
            y0 + (v1 - v0),
            u0,
            v0,
            u1,
            v1,
            1.0,
        );
    }

    // ---------------------------------------------------------------- hospital elevators (0x58cad0, 0x5888a0)
    fn draw_elevator(&mut self, tab: &[u8; 96], floor_mask: i32) {
        for i in 0..4usize {
            if (floor_mask & (1 << i)) != 0 {
                let x = read_i32(tab, (16 + i * 2) * 4) as f32 * 0.0625;
                let y = read_i32(tab, (16 + i * 2 + 1) * 4) as f32 * 0.0625;
                let u0 = read_i32(tab, i * 16) as f32;
                let v0 = read_i32(tab, i * 16 + 4) as f32;
                let u1 = read_i32(tab, i * 16 + 8) as f32;
                let v1 = read_i32(tab, i * 16 + 12) as f32;
                self.emit_rect(
                    "hsp/p_h_elevator_botan",
                    x,
                    y,
                    x + 64.0,
                    y + 80.0,
                    u0,
                    v0,
                    u1,
                    v1,
                    1.0,
                );
            }
        }
    }
}
