//! Exact 1:1 PS2 `ChizuMarkerDraw` (`SLUS_202.28` `0x001c7e90..0x001c824c`) bit-packed
//! red marker pen annotations extracted from `chizu_marker_list` (`0x0033d8d0`).
//!
//! # Binary Format (`SLUS_202.28` `0x001c8170..0x001c824c`)
//! Each 32-bit packed marker word encodes:
//! - Bits `0..6` (`word & 0x3f`): glyph palette index into `ptr0` (`[i16; 4]` atlas UV rect in px)
//! - Bits `6..19` (`((word >> 6) & 0x1fff) - 0x1000`): signed 13-bit Q4 fixed-point Y (`y_px = raw / 16.0`)
//! - Bits `19..32` (`((word >> 19) & 0x1fff) - 0x1000`): signed 13-bit Q4 fixed-point X (`x_px = raw / 16.0`)

use sh2_core::events::EventFlags;

/// Bit-packed marker record extracted directly from `SLUS_202.28` `chizu_marker_list` (`0x0033d8d0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChizuPackedMarker {
    /// 1-based map ID (`1..=35`).
    pub map_id: u8,
    /// `EventFlags` bit index enabling this marker (`flag_start + i` at `0x001c8134`).
    pub flag_id: u16,
    /// Exact 32-bit packed marker word from `ptr1[i - 1]` (`0x001c8170`).
    pub packed: u32,
    /// Exact `[u0, v0, u1, v1]` pixel rectangle inside `*mark.tex` from `ptr0[glyph_idx]` (`0x001c81fc`).
    pub uv_px: [i16; 4],
}

/// Decoded marker placement ready for `MapCanvasClipper` & `*mark.tex` atlas blitting.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChizuDecodedMarker {
    /// `EventFlags` bit index.
    pub flag_id: u16,
    /// Normalized sheet X (`0.0..=1.0`, mapped from PS2 centered `[-320..+320]`).
    pub norm_x: f32,
    /// Normalized sheet Y (`0.0..=1.0`, mapped from PS2 centered `[-240..+240]`).
    pub norm_y: f32,
    /// Glyph width in PS2 pixels (`u1 - u0`).
    pub width_px: f32,
    /// Glyph height in PS2 pixels (`v1 - v0`).
    pub height_px: f32,
    /// Atlas UV pixel rectangle `[u0, v0, u1, v1]`.
    pub uv_px: [i16; 4],
}

impl ChizuPackedMarker {
    /// Unpacks `self.packed` using the exact `ChizuMarkerDraw` (`0x001c8174..0x001c824c`) bit shifts.
    pub fn decode(&self) -> ChizuDecodedMarker {
        let y_raw = (((self.packed >> 6) & 0x1fff) as i32) - 0x1000;
        let x_raw = (((self.packed >> 19) & 0x1fff) as i32) - 0x1000;
        let x_px = (x_raw as f32) / 16.0;
        let y_px = (y_raw as f32) / 16.0;
        ChizuDecodedMarker {
            flag_id: self.flag_id,
            norm_x: (x_px + 320.0) / 640.0,
            norm_y: (y_px + 240.0) / 480.0,
            width_px: (self.uv_px[2] - self.uv_px[0]).abs().max(4) as f32,
            height_px: (self.uv_px[3] - self.uv_px[1]).abs().max(4) as f32,
            uv_px: self.uv_px,
        }
    }
}

/// Authentic binary marker entries extracted verbatim from `SLUS_202.28` `chizu_marker_list` (`0x0033d8d0`).
#[rustfmt::skip]
pub const CHIZU_MARKERS: &[ChizuPackedMarker] = &[
    // Map 1: Silent Hill Overall (0x0529..=0x052d, ptr0=0x0033d790, ptr1=0x0033d7f8)
    ChizuPackedMarker { map_id: 1, flag_id: 0x052a, packed: 0x6b84_f405, uv_px: [96, 1, 144, 81] },
    ChizuPackedMarker { map_id: 1, flag_id: 0x052b, packed: 0x6d05_0c08, uv_px: [176, 64, 207, 111] },
    ChizuPackedMarker { map_id: 1, flag_id: 0x052c, packed: 0x4e81_9c05, uv_px: [96, 1, 144, 81] },
    // Map 2: South Vale East (0x04e9..=0x0502, ptr0=0x0033d790, ptr1=0x0033d810)
    ChizuPackedMarker { map_id: 2, flag_id: 0x04ea, packed: 0x94ab_3c0a, uv_px: [192, 32, 240, 64] },
    ChizuPackedMarker { map_id: 2, flag_id: 0x04eb, packed: 0x29fe_9c8b, uv_px: [208, 64, 240, 128] },
    ChizuPackedMarker { map_id: 2, flag_id: 0x04ec, packed: 0x7293_d40a, uv_px: [192, 32, 240, 64] },
    ChizuPackedMarker { map_id: 2, flag_id: 0x04ed, packed: 0x7296_c80a, uv_px: [192, 32, 240, 64] },
    ChizuPackedMarker { map_id: 2, flag_id: 0x04ee, packed: 0x951d_b98a, uv_px: [192, 32, 240, 64] },
    ChizuPackedMarker { map_id: 2, flag_id: 0x04ef, packed: 0x951e_558a, uv_px: [192, 32, 240, 64] },
    ChizuPackedMarker { map_id: 2, flag_id: 0x04f0, packed: 0x480b_5eca, uv_px: [192, 32, 240, 64] },
    ChizuPackedMarker { map_id: 2, flag_id: 0x04f1, packed: 0x477e_f08a, uv_px: [192, 32, 240, 64] },
    // Map 3: South Vale West (0x0508..=0x051e, ptr0=0x0033d790, ptr1=0x0033d870)
    ChizuPackedMarker { map_id: 3, flag_id: 0x0509, packed: 0x8383_c000, uv_px: [96, 80, 144, 128] },
    ChizuPackedMarker { map_id: 3, flag_id: 0x050a, packed: 0x8383_c007, uv_px: [176, 1, 192, 49] },
    ChizuPackedMarker { map_id: 3, flag_id: 0x050b, packed: 0x9003_c800, uv_px: [96, 80, 144, 128] },
    ChizuPackedMarker { map_id: 3, flag_id: 0x050c, packed: 0xc824_cec2, uv_px: [0, 160, 80, 208] },
    ChizuPackedMarker { map_id: 3, flag_id: 0x050d, packed: 0xbe01_aa46, uv_px: [0, 1, 96, 161] },
    ChizuPackedMarker { map_id: 3, flag_id: 0x050e, packed: 0x5489_5c43, uv_px: [96, 128, 144, 240] },
    ChizuPackedMarker { map_id: 3, flag_id: 0x050f, packed: 0xde04_c80b, uv_px: [208, 64, 240, 128] },
    ChizuPackedMarker { map_id: 3, flag_id: 0x0510, packed: 0xda03_d3cb, uv_px: [208, 64, 240, 128] },
    // Map 4: Wood Side Apartments 1F (0x0533..=0x054a, ptr0=0x0033cc50, ptr1=0x0033cd90)
    ChizuPackedMarker { map_id: 4, flag_id: 0x0534, packed: 0xdeae_d681, uv_px: [0, 1, 15, 47] },
    ChizuPackedMarker { map_id: 4, flag_id: 0x0535, packed: 0xdc8e_f284, uv_px: [80, 32, 111, 47] },
    ChizuPackedMarker { map_id: 4, flag_id: 0x0536, packed: 0x7ed6_d681, uv_px: [0, 1, 15, 47] },
    ChizuPackedMarker { map_id: 4, flag_id: 0x0537, packed: 0x989d_acc0, uv_px: [128, 16, 159, 47] },
    ChizuPackedMarker { map_id: 4, flag_id: 0x0538, packed: 0x9a9c_6d87, uv_px: [112, 16, 127, 47] },
    ChizuPackedMarker { map_id: 4, flag_id: 0x0539, packed: 0x9a9b_5f87, uv_px: [112, 16, 127, 47] },
    ChizuPackedMarker { map_id: 4, flag_id: 0x053a, packed: 0x9882_4f40, uv_px: [128, 16, 159, 47] },
    ChizuPackedMarker { map_id: 4, flag_id: 0x053b, packed: 0xe1bd_ca07, uv_px: [112, 16, 127, 47] },
];

/// Returns all active decoded markers for `map_id` whose `EventFlags` bit is set (`ChizuMarkerDraw`).
pub fn active_markers_for_map(map_id: u8, flags: &EventFlags) -> Vec<ChizuDecodedMarker> {
    CHIZU_MARKERS
        .iter()
        .filter(|m| m.map_id == map_id && flags.get(m.flag_id as usize))
        .map(ChizuPackedMarker::decode)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chizu_packed_marker_bit_unpacking() {
        // Verify exact `SLUS_202.28` `0x001c8170` decode of Map 2 flag 0x04ea (`0x94ab3c0a`)
        let mut flags = EventFlags::new();
        flags.set(0x04ea, true);
        let marks = active_markers_for_map(2, &flags);
        assert_eq!(marks.len(), 1);
        let m = marks[0];
        assert_eq!(m.flag_id, 0x04ea);
        // x_px = +41.3125 -> (41.3125 + 320) / 640 = 0.56455
        // y_px = -49.0    -> (-49.0 + 240) / 480 = 0.39792
        assert!((m.norm_x - 0.56455).abs() < 1e-3);
        assert!((m.norm_y - 0.39792).abs() < 1e-3);
        assert_eq!(m.uv_px, [192, 32, 240, 64]);
    }
}
