//! Authentic 1:1 PS2 / Director's Cut `itemmenu2.tex` sprite atlas table (`TgsItemPitureDraw`)
//! and 3D circular carousel projection (`item_position` @ `SLUS_202.28` `0x001d3270`).
//!
//! # Ground Truth Citations
//! - `SLUS_202.28` `TgsItemPitureDraw` (`0x00209260`): `itemmenu2.tex` (`1024x1024` base grid,
//!   `128` px row height; scales cleanly to `4096x4096` SH2:EE HD `sh2e/pic/etc/itemmenu2.tex`).
//! - `sh2pc.exe` (Director's Cut) file offsets `0x3f37f8` (`[i16; 3]` `(u0, v0, u1)`) and
//!   `0x3f39d8` (`u32` Director's Cut item IDs matching `ITEM_CATALOG`).
//! - `SLUS_202.28` `item_position` (`0x001d3270..0x001d3488`):
//!   - `theta = (slot - 5.0) * PI / 5.0 / 1.6` (`0x40a00000`, `0x40490fdb`, `0x3fcccccd`)
//!   - `x_3d = sin(theta) * 1500.0` (`0x40977000_00000000`)
//!   - `z_3d = -cos(theta) * 1000.0` (`0x408f4000_00000000`)
//! - `SLUS_202.28` `itemmainmain` (`0x001ce050..0x001ce154`):
//!   - `depth_scale = (0.3 + (-z_3d / 1666.0)) * slot_scale` (`0x3e99999a`, `0x44d04000`)
//!   - `alpha = ((-z_3d - 300.0) / 5.5).max(0.0) / 128.0` (`0x43960000`, `0x40160000_00000000`).

/// Reference pixel dimension of `data/pic/etc/itemmenu2.tex` (`1024x1024`; HD `4096x4096` uses identical normalized UVs).
pub const ITEM_ATLAS_REF_SIZE: f32 = 1024.0;
/// Row height of each item icon cell in `itemmenu2.tex` (`128` px in `1024x1024` reference space).
pub const ITEM_ATLAS_ROW_HEIGHT: f32 = 128.0;

/// Single item sprite rectangle in `itemmenu2.tex` (`sh2pc.exe` `0x3f37f8` & `0x3f39d8`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemIconRect {
    /// Director's Cut `ITEM_CATALOG` item ID (`1..=85`).
    pub item_id: u8,
    /// Left U coordinate in `1024x1024` reference pixels.
    pub u0: u16,
    /// Top V coordinate in `1024x1024` reference pixels (`v1 = v0 + 128`).
    pub v0: u16,
    /// Right U coordinate in `1024x1024` reference pixels.
    pub u1: u16,
}

impl ItemIconRect {
    /// Normalized `(u0, v0, u1, v1)` UV bounds in `0.0..=1.0`, resolution-independent for
    /// both original `1024x1024` (`data/pic/etc/itemmenu2.tex`) and 4K HD (`sh2e/pic/etc/itemmenu2.tex`).
    pub const fn normalized_uv(&self) -> (f32, f32, f32, f32) {
        (
            self.u0 as f32 / ITEM_ATLAS_REF_SIZE,
            self.v0 as f32 / ITEM_ATLAS_REF_SIZE,
            self.u1 as f32 / ITEM_ATLAS_REF_SIZE,
            (self.v0 as f32 + ITEM_ATLAS_ROW_HEIGHT) / ITEM_ATLAS_REF_SIZE,
        )
    }

    /// Aspect ratio (`width / height`) of the sprite cell.
    pub fn aspect(&self) -> f32 {
        let w = (self.u1.saturating_sub(self.u0)).max(8) as f32;
        w / ITEM_ATLAS_ROW_HEIGHT
    }
}

/// Exact 75-entry Director's Cut `itemmenu2.tex` sprite table extracted verbatim from `sh2pc.exe` (`0x3f37f8` / `0x3f39d8`).
#[rustfmt::skip]
pub const ITEM_ICON_RECTS: &[ItemIconRect] = &[
    ItemIconRect { item_id: 12, u0:  20, v0:   0, u1:  97 },
    ItemIconRect { item_id:  6, u0: 123, v0:   0, u1: 225 },
    ItemIconRect { item_id:  8, u0: 236, v0:   0, u1: 338 },
    ItemIconRect { item_id: 14, u0: 363, v0:   0, u1: 436 },
    ItemIconRect { item_id: 15, u0: 460, v0:   0, u1: 564 },
    ItemIconRect { item_id: 16, u0: 570, v0:   0, u1: 680 },
    ItemIconRect { item_id:  4, u0: 700, v0:   0, u1: 770 },
    ItemIconRect { item_id: 24, u0: 790, v0:   0, u1: 868 },
    ItemIconRect { item_id: 13, u0: 900, v0:   0, u1: 954 },
    ItemIconRect { item_id:  5, u0:  26, v0: 128, u1: 100 },
    ItemIconRect { item_id:  7, u0: 120, v0: 128, u1: 214 },
    ItemIconRect { item_id:  9, u0: 214, v0: 128, u1: 311 },
    ItemIconRect { item_id: 65, u0: 330, v0: 128, u1: 422 },
    ItemIconRect { item_id: 20, u0: 435, v0: 128, u1: 527 },
    ItemIconRect { item_id: 18, u0: 560, v0: 128, u1: 604 },
    ItemIconRect { item_id: 70, u0: 640, v0: 128, u1: 722 },
    ItemIconRect { item_id: 57, u0: 745, v0: 128, u1: 808 },
    ItemIconRect { item_id: 56, u0: 825, v0: 128, u1: 900 },
    ItemIconRect { item_id: 55, u0: 937, v0: 128, u1: 981 },
    ItemIconRect { item_id: 51, u0:  24, v0: 256, u1:  96 },
    ItemIconRect { item_id: 52, u0: 118, v0: 256, u1: 185 },
    ItemIconRect { item_id: 50, u0: 208, v0: 256, u1: 286 },
    ItemIconRect { item_id: 60, u0: 298, v0: 256, u1: 386 },
    ItemIconRect { item_id: 61, u0: 397, v0: 256, u1: 480 },
    ItemIconRect { item_id: 59, u0: 490, v0: 256, u1: 578 },
    ItemIconRect { item_id: 68, u0: 586, v0: 256, u1: 690 },
    ItemIconRect { item_id: 67, u0: 696, v0: 256, u1: 787 },
    ItemIconRect { item_id: 69, u0: 794, v0: 256, u1: 900 },
    ItemIconRect { item_id: 64, u0: 922, v0: 256, u1: 992 },
    ItemIconRect { item_id: 19, u0:  12, v0: 384, u1: 112 },
    ItemIconRect { item_id: 66, u0: 137, v0: 384, u1: 196 },
    ItemIconRect { item_id: 27, u0: 220, v0: 384, u1: 304 },
    ItemIconRect { item_id: 44, u0: 323, v0: 384, u1: 414 },
    ItemIconRect { item_id: 43, u0: 424, v0: 384, u1: 515 },
    ItemIconRect { item_id: 48, u0: 535, v0: 384, u1: 616 },
    ItemIconRect { item_id: 46, u0: 633, v0: 384, u1: 728 },
    ItemIconRect { item_id: 38, u0: 738, v0: 384, u1: 826 },
    ItemIconRect { item_id: 40, u0: 840, v0: 384, u1: 935 },
    ItemIconRect { item_id: 28, u0:  26, v0: 512, u1: 104 },
    ItemIconRect { item_id: 29, u0: 120, v0: 512, u1: 215 },
    ItemIconRect { item_id: 37, u0: 224, v0: 512, u1: 318 },
    ItemIconRect { item_id: 30, u0: 330, v0: 512, u1: 420 },
    ItemIconRect { item_id: 45, u0: 425, v0: 512, u1: 517 },
    ItemIconRect { item_id: 33, u0: 520, v0: 512, u1: 613 },
    ItemIconRect { item_id: 42, u0: 618, v0: 512, u1: 708 },
    ItemIconRect { item_id: 47, u0: 712, v0: 512, u1: 822 },
    ItemIconRect { item_id: 26, u0: 828, v0: 512, u1: 918 },
    ItemIconRect { item_id: 39, u0:  12, v0: 640, u1: 101 },
    ItemIconRect { item_id: 31, u0: 122, v0: 640, u1: 195 },
    ItemIconRect { item_id: 32, u0: 218, v0: 640, u1: 307 },
    ItemIconRect { item_id: 35, u0: 323, v0: 640, u1: 390 },
    ItemIconRect { item_id: 36, u0: 425, v0: 640, u1: 500 },
    ItemIconRect { item_id: 34, u0: 526, v0: 640, u1: 599 },
    ItemIconRect { item_id: 41, u0: 616, v0: 640, u1: 708 },
    ItemIconRect { item_id: 71, u0: 836, v0: 640, u1: 904 },
    ItemIconRect { item_id: 76, u0: 938, v0: 640, u1: 980 },
    ItemIconRect { item_id: 63, u0:  28, v0: 768, u1:  96 },
    ItemIconRect { item_id: 74, u0: 121, v0: 768, u1: 195 },
    ItemIconRect { item_id: 75, u0: 216, v0: 768, u1: 292 },
    ItemIconRect { item_id: 54, u0: 312, v0: 768, u1: 376 },
    ItemIconRect { item_id: 53, u0: 394, v0: 768, u1: 475 },
    ItemIconRect { item_id:  1, u0: 506, v0: 768, u1: 565 },
    ItemIconRect { item_id: 49, u0: 590, v0: 768, u1: 690 },
    ItemIconRect { item_id: 21, u0: 712, v0: 768, u1: 805 },
    ItemIconRect { item_id:  3, u0: 824, v0: 768, u1: 904 },
    ItemIconRect { item_id: 25, u0: 916, v0: 768, u1:1001 },
    ItemIconRect { item_id: 77, u0:  24, v0: 896, u1: 100 },
    ItemIconRect { item_id: 72, u0: 138, v0: 896, u1: 180 },
    ItemIconRect { item_id: 73, u0: 214, v0: 896, u1: 264 },
    ItemIconRect { item_id:  2, u0: 300, v0: 896, u1: 404 },
    ItemIconRect { item_id: 62, u0: 424, v0: 896, u1: 500 },
    ItemIconRect { item_id: 58, u0: 522, v0: 896, u1: 598 },
    ItemIconRect { item_id: 23, u0: 618, v0: 896, u1: 708 },
    ItemIconRect { item_id: 22, u0: 724, v0: 896, u1: 820 },
    ItemIconRect { item_id: 78, u0: 843, v0: 896, u1: 898 },
];

/// Looks up the exact `itemmenu2.tex` sprite rectangle for `item_id`.
pub fn item_icon_rect(item_id: u8) -> Option<&'static ItemIconRect> {
    ITEM_ICON_RECTS.iter().find(|r| r.item_id == item_id)
}

/// Projected 2D slot placement computed from `SLUS_202.28` `item_position` (`0x001d3270`) &
/// `itemmainmain` (`0x001ce050..0x001ce154`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CarouselSlotProjection {
    /// Horizontal offset from screen center in 640x480 logical pixels.
    pub offset_x_640: f32,
    /// Depth-attenuated display scale (`~0.5..=0.9`).
    pub scale: f32,
    /// Depth-attenuated opacity (`0.0..=1.0`).
    pub alpha: f32,
}

/// Evaluates the exact `SLUS_202.28` `item_position` (`0x001d3378..0x001d3488`) + `itemmainmain`
/// (`0x001ce050..0x001ce154`) circular ring projection for `slot_offset` relative to center (`0`).
pub fn project_carousel_slot(slot_offset: f32, slot_scale: f32) -> CarouselSlotProjection {
    let theta = (slot_offset * std::f32::consts::PI) / 5.0 / 1.6;
    let x_3d = theta.sin() * 1500.0;
    let neg_z = theta.cos() * 1000.0; // -z_3d
                                      // Map PS2 `x_3d` (`[-1500..+1500]`) onto 640px wide carousel arc (`±240` px)
    let offset_x_640 = (x_3d / 1500.0) * 240.0;
    let depth_scale = (0.3 + (neg_z / 1666.0)) * slot_scale;
    let alpha = (((neg_z - 300.0) / 5.5).max(0.0) / 128.0).clamp(0.0, 1.0);
    CarouselSlotProjection {
        offset_x_640,
        scale: depth_scale,
        alpha,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_item_icon_atlas_and_carousel_ring_projection() {
        let radio = item_icon_rect(19).expect("Radio icon rect");
        assert_eq!((radio.u0, radio.v0, radio.u1), (12, 384, 112));
        let letter = item_icon_rect(21).expect("Letter from Mary icon rect");
        assert_eq!((letter.u0, letter.v0, letter.u1), (712, 768, 805));

        let center = project_carousel_slot(0.0, 1.0);
        assert!(center.offset_x_640.abs() < 1e-4);
        assert!(center.alpha > 0.95);
        assert!((center.scale - 0.9).abs() < 0.01);

        let side = project_carousel_slot(2.0, 0.8);
        assert!(side.offset_x_640 > 100.0);
        assert!(side.scale < center.scale);
        assert!(side.alpha < center.alpha);
    }
}
