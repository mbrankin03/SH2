//! Exact 1:1 PS2 / PC `ChizuMarkerDraw` (`SLUS_202.28` `0x001c7e90`, `sh2pc.exe` `0x0049b430`)
//! bit-packed red marker pen annotations and dual-layer combined-map lookup.
//!
//! # Binary Format (`SLUS_202.28` `0x001c8170..0x001c824c`, `sh2pc.exe` `0x0049b560..0x0049b730`)
//! Each 32-bit packed marker word encodes:
//! - Bits `0..6` (`word & 0x3f`): glyph palette index into `ptr0` (`[i16; 4]` atlas UV rect in px)
//! - Bits `6..19` (`((word >> 6) & 0x1fff) - 0x1000`): signed 13-bit Q4 fixed-point Y in PS2
//!   centered `[-256..+256]` virtual space (`y_ps2 = raw / 16.0`)
//! - Bits `19..32` (`((word >> 19) & 0x1fff) - 0x1000`): signed 13-bit Q4 fixed-point X in PS2
//!   centered `[-256..+256]` virtual space (`x_ps2 = raw / 16.0`)
//!
//! # Prototype vs. Retail Drift
//! `sh2pc.exe` (`0x0088e838`) shifts `EventFlags` marker indices by `+0xe4..+0xe7` relative to
//! `SLUS_202.28` (`0x0033d8d0`) and appends Map `36` (Baldwin Mansion, `0x0088ea78`). Because
//! `sh2-core`'s stage scripts use retail `sh2pc.exe` flag IDs, [`CHIZU_MARKERS`] stores the full
//! 908-entry retail `sh2pc.exe` table.

use sh2_core::events::EventFlags;

pub use crate::map::markers_data::CHIZU_MARKERS;

/// Bit-packed marker record extracted directly from `sh2pc.exe` `chizu_marker_list` (`0x0088e838`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChizuPackedMarker {
    /// 1-based map layer ID (`1..=36`).
    pub map_id: u8,
    /// `EventFlags` bit index enabling this marker (`flag_start + i` at `0x001c8134`).
    pub flag_id: u16,
    /// Exact 32-bit packed marker word from `ptr1[i - 1]` (`0x001c8170`).
    pub packed: u32,
    /// Exact `[u0, v0, u1, v1]` pixel rectangle inside `*mark.tex` from `ptr0[glyph_idx]` (`0x001c81fc`).
    pub uv_px: [i16; 4],
}

/// Decoded marker placement ready for `ChizuMarkerDraw` (`0x001c7e90`) & `*mark.tex` atlas blitting.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChizuDecodedMarker {
    /// `EventFlags` bit index.
    pub flag_id: u16,
    /// Top-left anchor X in PS2 centered `[-256..+256]` virtual pixels (`vitof4` at `0x001c8184`).
    pub x_ps2: f32,
    /// Top-left anchor Y in PS2 centered `[-256..+256]` virtual pixels (`vitof4` at `0x001c8194`).
    pub y_ps2: f32,
    /// Normalized sheet X (`0.0..=1.0`, derived as `(x_ps2 + 256.0) / 512.0`).
    pub norm_x: f32,
    /// Normalized sheet Y (`0.0..=1.0`, derived as `(y_ps2 + 256.0) / 512.0`).
    pub norm_y: f32,
    /// Atlas glyph width in texture pixels (`u1 - u0`).
    pub width_px: f32,
    /// Atlas glyph height in texture pixels (`v1 - v0`).
    pub height_px: f32,
    /// Atlas UV pixel rectangle `[u0, v0, u1, v1]`.
    pub uv_px: [i16; 4],
}

impl ChizuPackedMarker {
    /// Unpacks `self.packed` using the exact `ChizuMarkerDraw` (`0x001c8174..0x001c824c`) bit shifts.
    #[must_use]
    pub fn decode(&self) -> ChizuDecodedMarker {
        let y_raw = (((self.packed >> 6) & 0x1fff) as i32) - 0x1000;
        let x_raw = (((self.packed >> 19) & 0x1fff) as i32) - 0x1000;
        let x_ps2 = (x_raw as f32) / 16.0;
        let y_ps2 = (y_raw as f32) / 16.0;
        ChizuDecodedMarker {
            flag_id: self.flag_id,
            x_ps2,
            y_ps2,
            norm_x: (x_ps2 + 256.0) / 512.0,
            norm_y: (y_ps2 + 256.0) / 512.0,
            width_px: (self.uv_px[2] - self.uv_px[0]) as f32,
            height_px: (self.uv_px[3] - self.uv_px[1]) as f32,
            uv_px: self.uv_px,
        }
    }
}

/// Returns the 1 or 2 `chizu_marker_list` layer IDs rendered for `map_id`
/// (`ChizuMarkerDraw` `SLUS_202.28` `0x001c7fd4..0x001c80f8`, `sh2pc.exe` `0x0049b496..0x0049b555`).
///
/// Combined / shared-sheet maps draw two marker layers in order:
/// - `7, 8` (Blue Creek 1F/2F on shared `apartmapw.tex`): `[7, 8]`
/// - `9` (Combined Apartments 1F): `[4, 9]` (Wood Side 1F + Blue Creek 1F)
/// - `10` (Combined Apartments 2F): `[5, 10]` (Wood Side 2F + Blue Creek 2F)
/// - `16, 17` (Toluca Prison + Courtyard): `[16, 17]`
/// - `28` (Lakeview Hotel Emp BF): `[28, 20]` (Employee BF + Guest BF)
/// - `29` (Lakeview Hotel Emp 1F): `[29, 21]` (Employee 1F + Guest 1F)
/// - `32` (Hotel Otherworld Emp BF): `[32, 24]` (Otherworld Emp BF + Guest BF)
/// - `33` (Hotel Otherworld Emp 1F): `[33, 25]` (Otherworld Emp 1F + Guest 1F)
#[must_use]
pub const fn chizu_marker_layers(map_id: u8) -> (u8, Option<u8>) {
    match map_id {
        7 | 8 => (7, Some(8)),
        9 => (4, Some(9)),
        10 => (5, Some(10)),
        16 | 17 => (16, Some(17)),
        28 => (28, Some(20)),
        29 => (29, Some(21)),
        32 => (32, Some(24)),
        33 => (33, Some(25)),
        other => (other, None),
    }
}

/// Returns all active decoded markers for `map_id` whose `EventFlags` bit is set, including both
/// layers for dual-layer combined maps (`ChizuMarkerDraw` `0x001c7e90` / `sh2pc.exe` `0x0049b430`).
#[must_use]
pub fn active_markers_for_map(map_id: u8, flags: &EventFlags) -> Vec<ChizuDecodedMarker> {
    if map_id == 0 {
        return Vec::new();
    }
    let (layer0, layer1) = chizu_marker_layers(map_id);
    let mut out = Vec::new();
    for layer_id in [Some(layer0), layer1].into_iter().flatten() {
        for m in CHIZU_MARKERS
            .iter()
            .filter(|m| m.map_id == layer_id && flags.get(m.flag_id as usize))
        {
            out.push(m.decode());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chizu_packed_marker_bit_unpacking_and_full_table() {
        assert_eq!(CHIZU_MARKERS.len(), 908);
        // Verify Map 2 first retail marker (flag 0x05ce = sh2pc.exe 0x0088e608, SLUS 0x04ea)
        let mut flags = EventFlags::new();
        flags.set(0x05ce, true);
        let marks = active_markers_for_map(2, &flags);
        assert_eq!(marks.len(), 1);
        let m = marks[0];
        assert_eq!(m.flag_id, 0x05ce);
        // x_ps2 = +41.3125, y_ps2 = -49.0 -> (41.3125 + 256)/512 = 0.5806885, (-49 + 256)/512 = 0.4042969
        assert!((m.x_ps2 - 41.3125).abs() < 1e-4);
        assert!((m.y_ps2 - (-49.0)).abs() < 1e-4);
        assert!((m.norm_x - (41.3125 + 256.0) / 512.0).abs() < 1e-5);
        assert!((m.norm_y - (-49.0 + 256.0) / 512.0).abs() < 1e-5);
        assert_eq!(m.uv_px, [192, 32, 240, 64]);
    }

    #[test]
    fn test_chizu_dual_layer_combined_maps() {
        assert_eq!(chizu_marker_layers(7), (7, Some(8)));
        assert_eq!(chizu_marker_layers(8), (7, Some(8)));
        assert_eq!(chizu_marker_layers(9), (4, Some(9)));
        assert_eq!(chizu_marker_layers(10), (5, Some(10)));
        assert_eq!(chizu_marker_layers(16), (16, Some(17)));
        assert_eq!(chizu_marker_layers(17), (16, Some(17)));
        assert_eq!(chizu_marker_layers(28), (28, Some(20)));
        assert_eq!(chizu_marker_layers(29), (29, Some(21)));
        assert_eq!(chizu_marker_layers(32), (32, Some(24)));
        assert_eq!(chizu_marker_layers(33), (33, Some(25)));
        assert_eq!(chizu_marker_layers(36), (36, None));

        // Combined Apartments 1F (Map 9) draws both Map 4 (Wood Side 1F) and Map 9 (Blue Creek 1F)
        let mut flags = EventFlags::new();
        flags.set(0x061b, true); // Map 4 marker
        flags.set(0x0680, true); // Map 9 marker
        let marks = active_markers_for_map(9, &flags);
        assert_eq!(marks.len(), 2);
        assert_eq!(marks[0].flag_id, 0x061b);
        assert_eq!(marks[1].flag_id, 0x0680);

        // Hotel Employee 1F (Map 29) draws both Map 29 and Map 21 (Guest 1F)
        let mut hflags = EventFlags::new();
        hflags.set(0x084a, true); // Map 29 marker
        hflags.set(0x082e, true); // Map 21 marker
        let hmarks = active_markers_for_map(29, &hflags);
        assert_eq!(hmarks.len(), 2);
        assert_eq!(hmarks[0].flag_id, 0x084a);
        assert_eq!(hmarks[1].flag_id, 0x082e);
    }
}
