//! PS2 Outdoor Tile Grid Coordinate Math & Retail Block Tables (`BlockNumber` `0x001b09f0`).
//!
//! Verified 1:1 against `SLUS_202.28` `BlockNumber` (`0x001b09f0..0x001b0cc4`) and `sh2pc.exe`
//! (`0x0048b980`):
//! - Tile cell width & depth: `20,000.0 mm` (`0x469c4000` at `0x001b0b08`).
//! - Half-tile extent: `10,000.0 mm` (`0x461c4000` at `0x00271718`).
//! - Packed 10-bit lookup key: `((gx + 16) << 5) | (gz + 16)`.

/// Outdoor `.map` tile width and depth in PS2 world millimeters (`0x469c4000` in `BlockNumber` `0x001b0b08`).
pub const OUTDOOR_TILE_SIZE_MM: f32 = 20_000.0;

/// Half of an outdoor tile extent in PS2 world millimeters (`0x461c4000` in `loadBgAll_PrepareAround` `0x00271718`).
pub const OUTDOOR_TILE_HALF_MM: f32 = 10_000.0;

/// Grid coordinate bias applied when packing `(gx, gz)` into the 10-bit `BlockNumber` key (`0x001b0b90`).
pub const OUTDOOR_GRID_OFFSET: i16 = 16;

/// Forest Trail & Graveyard (`ca`, `Stage 4`) outdoor `(gx, gz, block_no)` table (`SLUS_202.28` `0x002b9df0`).
pub const CA_OUTDOOR_BLOCKS: &[(i16, i16, u16)] = &[
    (1, -4, 3),
    (1, -5, 4),
    (0, 0, 10),
    (0, -1, 11),
    (0, -2, 12),
    (0, -3, 9),
    (0, -4, 8),
    (-1, 0, 13),
    (-1, -1, 14),
    (-2, 0, 15),
    (-2, -1, 16),
    (-3, 0, 17),
    (-3, -1, 18),
    (-4, 0, 19),
    (-4, -1, 20),
    (-5, 0, 21),
    (-5, -1, 22),
    (-6, 0, 23),
    (-6, -1, 24),
    (-7, -1, 26),
    (-7, -2, 27),
    (-8, -1, 25),
    (-8, -2, 28),
];

/// Silent Hill Town East (`cb`, `Stage 5`) outdoor `(gx, gz, block_no)` table (`SLUS_202.28` `0x002b9e50`).
pub const CB_OUTDOOR_BLOCKS: &[(i16, i16, u16)] = &[
    (5, 1, 64),
    (5, -4, 69),
    (4, 1, 62),
    (4, 0, 63),
    (4, -4, 67),
    (4, -5, 68),
    (3, 0, 60),
    (3, -1, 61),
    (3, -4, 65),
    (3, -5, 66),
    (2, 2, 54),
    (2, 1, 55),
    (2, -1, 57),
    (2, -2, 59),
    (2, -3, 58),
    (1, 1, 49),
    (1, 0, 50),
    (1, -1, 51),
    (1, -2, 52),
    (1, -3, 53),
    (0, 1, 2),
    (0, 0, 3),
    (0, -1, 4),
    (0, -2, 5),
    (0, -3, 7),
    (-1, 1, 10),
    (-1, 0, 11),
    (-1, -1, 12),
    (-1, -2, 13),
    (-1, -3, 14),
    (-2, 1, 18),
    (-2, 0, 19),
    (-2, -1, 20),
    (-2, -2, 21),
    (-2, -3, 22),
    (-3, 2, 25),
    (-3, 1, 26),
    (-3, 0, 27),
    (-3, -1, 28),
    (-3, -2, 29),
    (-3, -3, 30),
    (-3, -4, 31),
    (-4, 2, 33),
    (-4, 1, 34),
    (-4, 0, 35),
    (-4, -1, 36),
    (-4, -2, 37),
    (-4, -3, 38),
    (-4, -4, 39),
    (-5, 0, 41),
    (-5, -1, 42),
    (-5, -3, 44),
    (-5, -4, 47),
    (-6, 0, 46),
    (-6, -1, 45),
];

/// Silent Hill Town West & Rosewater Park (`cc`, `Stage 14` / `54`) outdoor `(gx, gz, block_no)` table
/// (`SLUS_202.28` `0x002b9f30`).
pub const CC_OUTDOOR_BLOCKS: &[(i16, i16, u16)] = &[
    (3, -3, 77),
    (3, -4, 76),
    (2, -3, 75),
    (2, -4, 74),
    (1, 5, 1),
    (1, 4, 2),
    (1, 3, 3),
    (1, 2, 4),
    (1, 1, 5),
    (1, -3, 6),
    (1, -4, 68),
    (0, 5, 7),
    (0, 4, 8),
    (0, 3, 9),
    (0, 2, 10),
    (0, 1, 11),
    (0, 0, 12),
    (0, -1, 24),
    (0, -2, 14),
    (0, -3, 15),
    (0, -4, 16),
    (-1, 5, 17),
    (-1, 4, 18),
    (-1, 3, 19),
    (-1, 2, 20),
    (-1, 1, 21),
    (-1, 0, 22),
    (-1, -1, 23),
    (-1, -2, 13),
    (-1, -3, 25),
    (-1, -4, 26),
    (-2, 5, 27),
    (-2, 4, 28),
    (-2, 3, 29),
    (-2, 2, 30),
    (-2, 0, 31),
    (-2, -1, 32),
    (-2, -2, 33),
    (-2, -3, 34),
    (-3, 3, 35),
    (-3, 2, 36),
    (-3, 0, 37),
    (-3, -1, 38),
    (-3, -2, 39),
    (-3, -3, 40),
    (-4, 3, 41),
    (-4, 2, 42),
    (-4, 1, 43),
    (-4, 0, 44),
    (-4, -1, 45),
    (-4, -2, 46),
    (-4, -3, 47),
    (-5, 4, 48),
    (-5, 3, 49),
    (-5, 2, 50),
    (-5, 1, 51),
    (-5, 0, 52),
    (-5, -1, 53),
    (-5, -2, 54),
    (-5, -3, 55),
    (-6, 4, 56),
    (-6, 3, 57),
    (-7, 5, 73),
    (-7, 4, 60),
    (-7, 3, 61),
    (-8, 5, 62),
    (-8, 4, 63),
    (-9, 5, 64),
    (-9, 4, 65),
    (-9, -2, 99),
    (-10, 6, 66),
    (-10, 5, 67),
    (-11, 6, 69),
    (-11, 5, 70),
    (-12, 6, 71),
    (-12, 5, 72),
];

/// Silent Hill Town Dark (`cd`, `Stage 29`) outdoor `(gx, gz, block_no)` table (`SLUS_202.28` `0x002ba060`).
pub const CD_OUTDOOR_BLOCKS: &[(i16, i16, u16)] = &[
    (0, -1, 4),
    (-1, 0, 2),
    (-1, -1, 5),
    (-1, -6, 1),
    (-2, -1, 6),
    (-3, -4, 11),
];

/// Toluca Lake (`qt`, `Stage 38`) outdoor `(gx, gz, block_no)` table.
pub const QT_OUTDOOR_BLOCKS: &[(i16, i16, u16)] = &[(0, 0, 1)];

/// Outdoor area identifier corresponding to PS2 `BgIsOut` (`0x001b0cd0`) areas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OutdoorArea {
    /// Forest Trail & Graveyard (`ca`, `Stage 4`).
    Ca,
    /// Silent Hill Town East (`cb`, `Stage 5`).
    Cb,
    /// Silent Hill Town West & Rosewater Park (`cc`, `Stage 14` & `Stage 54` BFW Town).
    Cc,
    /// Silent Hill Town Dark (`cd`, `Stage 29`).
    Cd,
    /// Toluca Lake (`qt`, `Stage 38`).
    Qt,
}

impl OutdoorArea {
    /// Resolves an outdoor area from a retail `stage_id`, returning `None` for indoor stages.
    #[must_use]
    pub const fn from_stage_id(stage_id: u16) -> Option<Self> {
        match stage_id {
            4 => Some(Self::Ca),
            5 => Some(Self::Cb),
            14 | 54 => Some(Self::Cc),
            29 => Some(Self::Cd),
            38 => Some(Self::Qt),
            _ => None,
        }
    }

    /// Two-letter directory prefix under `data/bg/` (e.g. `"ca"`, `"cb"`, `"cc"`, `"cd"`, `"qt"`).
    #[inline]
    #[must_use]
    pub const fn prefix(self) -> &'static str {
        match self {
            Self::Ca => "ca",
            Self::Cb => "cb",
            Self::Cc => "cc",
            Self::Cd => "cd",
            Self::Qt => "qt",
        }
    }

    /// Canonical relative path to the area's global `<area>GB.cam` camera road file, if present.
    #[inline]
    #[must_use]
    pub const fn global_cam_rel_path(self) -> Option<&'static str> {
        match self {
            Self::Ca => Some("bg/ca/caGB.cam"),
            Self::Cb => Some("bg/cb/cbGB.cam"),
            Self::Cc => Some("bg/cc/ccGB.cam"),
            Self::Cd => Some("bg/cd/cdGB.cam"),
            Self::Qt => None,
        }
    }

    /// Returns the retail `(gx, gz, block_no)` tile table for this outdoor area.
    #[inline]
    #[must_use]
    pub const fn block_table(self) -> &'static [(i16, i16, u16)] {
        match self {
            Self::Ca => CA_OUTDOOR_BLOCKS,
            Self::Cb => CB_OUTDOOR_BLOCKS,
            Self::Cc => CC_OUTDOOR_BLOCKS,
            Self::Cd => CD_OUTDOOR_BLOCKS,
            Self::Qt => QT_OUTDOOR_BLOCKS,
        }
    }

    /// Converts world `(x_mm, z_mm)` into `(gx, gz)` grid coordinates using exact `SLUS_202.28`
    /// `BlockNumber` (`0x001b0ae0..0x001b0b8c`) truncation + negative-quadrant bias.
    #[must_use]
    pub fn world_to_grid(x_mm: f32, z_mm: f32) -> (i16, i16) {
        let gx = (x_mm / OUTDOOR_TILE_SIZE_MM).floor() as i16;
        let gz = (z_mm / OUTDOOR_TILE_SIZE_MM).floor() as i16;
        (gx, gz)
    }

    /// Returns the world-space center `(cx_mm, cz_mm)` of grid cell `(gx, gz)` in PS2 mm.
    #[inline]
    #[must_use]
    pub fn grid_cell_center_mm(gx: i16, gz: i16) -> (f32, f32) {
        (
            f32::from(gx) * OUTDOOR_TILE_SIZE_MM + OUTDOOR_TILE_HALF_MM,
            f32::from(gz) * OUTDOOR_TILE_SIZE_MM + OUTDOOR_TILE_HALF_MM,
        )
    }

    /// Looks up the retail `block_no` for grid cell `(gx, gz)` in this area.
    #[must_use]
    pub fn block_no_at(self, gx: i16, gz: i16) -> Option<u16> {
        self.block_table()
            .iter()
            .find(|&&(tx, tz, _)| tx == gx && tz == gz)
            .map(|&(_, _, b)| b)
    }

    /// Looks up the `(gx, gz)` cell for a given `block_no` in this area.
    #[must_use]
    pub fn grid_for_block(self, block_no: u16) -> Option<(i16, i16)> {
        self.block_table()
            .iter()
            .find(|&&(_, _, b)| b == block_no)
            .map(|&(tx, tz, _)| (tx, tz))
    }

    /// Formats the canonical relative `.map` path for `block_no` (e.g. `"bg/cb/cb29.map"`).
    #[must_use]
    pub fn map_rel_path(self, block_no: u16) -> String {
        format!("bg/{}/{}{:02}.map", self.prefix(), self.prefix(), block_no)
    }
}

/// Identifies a specific outdoor `.map` tile within an [`OutdoorArea`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OutdoorTileId {
    /// Outdoor area (`Ca`, `Cb`, `Cc`, `Cd`, `Qt`).
    pub area: OutdoorArea,
    /// Numeric block index (e.g. `29` for `cb29.map`).
    pub block_no: u16,
    /// Grid X coordinate (`floor(x_mm / 20000.0)`).
    pub gx: i16,
    /// Grid Z coordinate (`floor(z_mm / 20000.0)`).
    pub gz: i16,
}

impl OutdoorTileId {
    /// Relative `.map` path under `data/` (e.g. `"bg/cb/cb29.map"`).
    #[must_use]
    pub fn map_rel_path(self) -> String {
        self.area.map_rel_path(self.block_no)
    }
}
