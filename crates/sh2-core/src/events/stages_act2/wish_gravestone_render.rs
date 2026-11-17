//! *Born from a Wish* Baldwin Mansion gravestone board identities, rotations, and 2D cutaway
//! presentation geometry (UVs and quad vertices) with the SH2:EE `GravestoneBoardsFix` toggle.
//!
//! Vanilla values verified 1:1 against retail PC `sh2pc.exe` (UV table `0x008c4650`, vertex
//! extents `0x008c4670`, assembled at `0x00571fcd..0x005720ec`); fixed values from SH2:EE
//! `PuzzleAlignmentFixes.cpp:9-96`.

use super::{ITEM_BLACK_BOARD, ITEM_RED_BOARD, ITEM_WHITE_BOARD};

/// Which of the three *Born from a Wish* gravestone boards is being manipulated (`0x00572d40`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GravestoneBoard {
    /// Item `79` (`0x4f`, pickup model `0x074e`, placed model `0x0753`): "White Board".
    White,
    /// Item `80` (`0x50`, pickup model `0x074c`, placed model `0x0754`): "Black Board".
    Black,
    /// Item `81` (`0x51`, pickup model `0x074d`, placed model `0x0755`): "Red Board".
    Red,
}

impl GravestoneBoard {
    /// Inventory item ID (`79..=81`).
    pub const fn item_id(self) -> u8 {
        match self {
            Self::Black => ITEM_BLACK_BOARD,
            Self::White => ITEM_WHITE_BOARD,
            Self::Red => ITEM_RED_BOARD,
        }
    }
}

/// 90-degree board orientation in the Baldwin Mansion gravestone cavity (`0x00572fb0..0x00573068`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BoardRotation {
    /// Choice `0`: Upright (`0°`).
    Deg0 = 0,
    /// Choice `1`: Rotated right (`-90°` / `270°`, `0x00572fde`).
    Right90 = 1,
    /// Choice `2`: Upside-down (`180°`, `0x00573019`).
    Deg180 = 2,
    /// Choice `3`: Rotated left (`+90°`, `0x00573044`).
    Left90 = 3,
}

/// UV rectangle `[u1, v1, u2, v2]` in texel space (`sh2pc.exe` `0x008c4650`, SH2:EE `GravestoneBoardsFix`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GravestoneBoardUv {
    /// Left U coordinate in `1/16` texels (`+0x1c` on `UIElement` at `0x01f7a77c`).
    pub u1: f32,
    /// Top V coordinate in `1/16` texels (`+0x20` on `UIElement` at `0x01f7a780`).
    pub v1: f32,
    /// Right U coordinate in `1/16` texels (`+0x24` on `UIElement` at `0x01f7a784`).
    pub u2: f32,
    /// Bottom V coordinate in `1/16` texels (`+0x28` on `UIElement` at `0x01f7a788`).
    pub v2: f32,
}

/// Returns either the SH2:EE-aligned (`PuzzleAlignmentFixes.cpp:17-41`) or 1:1 vanilla PC
/// (`sh2pc.exe` `0x008c4650..0x008c4667` read at `0x00572148`) UV coordinates for `board`.
pub const fn gravestone_board_uv(board: GravestoneBoard, alignment_fix: bool) -> GravestoneBoardUv {
    if alignment_fix {
        match board {
            GravestoneBoard::Black => GravestoneBoardUv {
                u1: 3.0,
                v1: 1.0,
                u2: 2548.0,
                v2: 3320.0,
            },
            GravestoneBoard::White => GravestoneBoardUv {
                u1: 2581.0,
                v1: -5.0,
                u2: 5126.0,
                v2: 3320.0,
            },
            GravestoneBoard::Red => GravestoneBoardUv {
                u1: 3.0,
                v1: 3316.0,
                u2: 2542.0,
                v2: 6636.0,
            },
        }
    } else {
        match board {
            GravestoneBoard::White => GravestoneBoardUv {
                u1: 2560.0,
                v1: 0.0,
                u2: 5104.0,
                v2: 3312.0,
            },
            GravestoneBoard::Black => GravestoneBoardUv {
                u1: 0.0,
                v1: 0.0,
                u2: 2544.0,
                v2: 3312.0,
            },
            GravestoneBoard::Red => GravestoneBoardUv {
                u1: 0.0,
                v1: 3312.0,
                u2: 2544.0,
                v2: 6640.0,
            },
        }
    }
}

/// Vanilla gravestone board quad extents `[x0, y0, x1, y1]` (`sh2pc.exe` `0x008c4670`, `i16`).
pub const GRAVESTONE_BOARD_VERT_EXTENTS: [i16; 4] = [-1285, -2110, 1275, 1218];

/// Returns the four `(x, y)` screen-space quad vertices (`UIElement` `0x01f7a76c..0x01f7a77a`) of a
/// gravestone board at `rotation`.
///
/// Vanilla (`alignment_fix == false`) orderings assembled from `GRAVESTONE_BOARD_VERT_EXTENTS`
/// (`X0, Y0, X1, Y1`) by `sh2pc.exe`:
/// - `Deg0` (`0x00572058`): `(X0,Y0) (X1,Y0) (X0,Y1) (X1,Y1)`
/// - `Right90` (`0x00572092`): `(X1,Y0) (X1,Y1) (X0,Y0) (X0,Y1)`
/// - `Deg180` (`0x005720be`): `(X1,Y1) (X0,Y1) (X1,Y0) (X0,Y0)`
/// - `Left90` (`0x00571fcd`): `(X0,Y1) (X0,Y0) (X1,Y1) (X1,Y0)`
///
/// With `alignment_fix == true`, the three rotated orientations use SH2:EE
/// `PuzzleAlignmentFixes.cpp:50-94` vertices; `Deg0` is unchanged.
pub const fn gravestone_board_verts(rotation: BoardRotation, alignment_fix: bool) -> [[i16; 2]; 4] {
    let [x0, y0, x1, y1] = GRAVESTONE_BOARD_VERT_EXTENTS;
    match (rotation, alignment_fix) {
        (BoardRotation::Deg0, _) => [[x0, y0], [x1, y0], [x0, y1], [x1, y1]],
        (BoardRotation::Right90, false) => [[x1, y0], [x1, y1], [x0, y0], [x0, y1]],
        (BoardRotation::Deg180, false) => [[x1, y1], [x0, y1], [x1, y0], [x0, y0]],
        (BoardRotation::Left90, false) => [[x0, y1], [x0, y0], [x1, y1], [x1, y0]],
        (BoardRotation::Right90, true) => {
            [[1355, -2030], [1355, 1158], [-1313, -2030], [-1313, 1158]]
        }
        (BoardRotation::Deg180, true) => {
            [[1287, 1262], [-1267, 1262], [1287, -2066], [-1267, -2066]]
        }
        (BoardRotation::Left90, true) => {
            [[-1353, 1176], [-1353, -2005], [1315, 1176], [1315, -2005]]
        }
    }
}
