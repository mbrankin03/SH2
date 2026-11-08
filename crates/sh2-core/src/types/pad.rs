use bytemuck::{Pod, Zeroable};

/// 3D Tank-Control Input State (`PAD_3D`, 6 bytes, DIE `@0xe843`)
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub struct Pad3d {
    pub round_way: i8, // +0x00 (0) Turning direction
    pub reserved1: u8, // +0x01 (1) Reserved padding
    pub lslide: u8,    // +0x02 (2) Left strafe / slide pressure
    pub rslide: u8,    // +0x03 (3) Right strafe / slide pressure
    pub lturn180: u8,  // +0x04 (4) Left 180-degree quick-turn trigger
    pub rturn180: u8,  // +0x05 (5) Right 180-degree quick-turn trigger
}

#[allow(non_camel_case_types)]
pub type PAD_3D = Pad3d;

/// 2D Camera-Relative Control Input State (`PAD_2D`, 8 bytes, DIE `@0xeaa6`)
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct Pad2d {
    pub pow: u8,          // +0x00 (0) Movement magnitude
    pub lower_motion: u8, // +0x01 (1) Lower-body motion state
    pub _pad: [u8; 2],    // +0x02 (2) Alignment padding
    pub dir: f32,         // +0x04 (4) Movement direction angle (radians)
}

#[allow(non_camel_case_types)]
pub type PAD_2D = Pad2d;

/// Processed Controller Frame Snapshot (`PAD_INFO`, 32 bytes, DIE `@0xed62`)
/// Stored in a 10-element history ring buffer (`pad[10]`, 320 bytes) inside `PlayerState`.
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct PadInfo {
    pub action: u8,       // +0x00 (0)  Action / confirm button
    pub dash: u8,         // +0x01 (1)  Run / dash button
    pub attack0: u8,      // +0x02 (2)  Primary attack input
    pub attack1: u8,      // +0x03 (3)  Secondary attack input
    pub attack2: u8,      // +0x04 (4)  Tertiary attack input
    pub menu: u8,         // +0x05 (5)  Inventory / status screen button
    pub hold: u8,         // +0x06 (6)  Weapon ready / aim hold button
    pub search: u8,       // +0x07 (7)  Search / camera view button
    pub map: u8,          // +0x08 (8)  Map toggle button
    pub light: u8,        // +0x09 (9)  Flashlight toggle nibbles (`light_:4 | light:4`)
    pub l_button: u8,     // +0x0A (10) Left shoulder button
    pub r_button: u8,     // +0x0B (11) Right shoulder button
    pub lstick_x: i8,     // +0x0C (12) Left analog stick X (-128..127)
    pub lstick_y: i8,     // +0x0D (13) Left analog stick Y (-128..127)
    pub rstick_x: i8,     // +0x0E (14) Right analog stick X (-128..127)
    pub rstick_y: i8,     // +0x0F (15) Right analog stick Y (-128..127)
    pub forward: u8,      // +0x10 (16) Move forward pressure
    pub backward: u8,     // +0x11 (17) Move backward pressure
    pub lround: u8,       // +0x12 (18) Turn left pressure
    pub rround: u8,       // +0x13 (19) Turn right pressure
    pub skip: u8,         // +0x14 (20) Cutscene skip button
    pub pause: u8,        // +0x15 (21) Pause button
    pub _pad: [u8; 2],    // +0x16 (22) Alignment padding before +0x18 union
    pub mode_data: Pad2d, // +0x18 (24) 8-byte union storage for `PAD_3D` (6B) / `PAD_2D` (8B)
}

impl PadInfo {
    /// Low 4-bit nibble (`light_` bitfield in DWARF-1).
    #[inline]
    pub const fn light_low(&self) -> u8 {
        self.light & 0x0F
    }

    /// High 4-bit nibble (`light` bitfield in DWARF-1).
    #[inline]
    pub const fn light_high(&self) -> u8 {
        (self.light >> 4) & 0x0F
    }

    /// Zero-copy view of the `+0x18` union as `Pad3d` (`PAD_3D`, 6 bytes).
    #[inline]
    pub fn as_pad3d(&self) -> &Pad3d {
        let bytes = &bytemuck::bytes_of(&self.mode_data)[..6];
        bytemuck::from_bytes(bytes)
    }

    /// Mutable zero-copy view of the `+0x18` union as `Pad3d` (`PAD_3D`, 6 bytes).
    #[inline]
    pub fn as_pad3d_mut(&mut self) -> &mut Pad3d {
        let bytes = &mut bytemuck::bytes_of_mut(&mut self.mode_data)[..6];
        bytemuck::from_bytes_mut(bytes)
    }

    /// Zero-copy view of the `+0x18` union as `Pad2d` (`PAD_2D`, 8 bytes).
    #[inline]
    pub fn as_pad2d(&self) -> &Pad2d {
        &self.mode_data
    }

    /// Mutable zero-copy view of the `+0x18` union as `Pad2d` (`PAD_2D`, 8 bytes).
    #[inline]
    pub fn as_pad2d_mut(&mut self) -> &mut Pad2d {
        &mut self.mode_data
    }
}

#[allow(non_camel_case_types)]
pub type PAD_INFO = PadInfo;

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{offset_of, size_of};

    #[test]
    fn test_pad_sizes_and_offsets() {
        assert_eq!(size_of::<Pad3d>(), 6);
        assert_eq!(size_of::<Pad2d>(), 8);
        assert_eq!(offset_of!(Pad2d, dir), 0x04);

        assert_eq!(size_of::<PadInfo>(), 32);
        assert_eq!(offset_of!(PadInfo, action), 0x00);
        assert_eq!(offset_of!(PadInfo, light), 0x09);
        assert_eq!(offset_of!(PadInfo, lstick_x), 0x0C);
        assert_eq!(offset_of!(PadInfo, forward), 0x10);
        assert_eq!(offset_of!(PadInfo, pause), 0x15);
        assert_eq!(offset_of!(PadInfo, mode_data), 0x18);
    }

    #[test]
    fn test_pad_union_accessors() {
        let mut pad = PadInfo::default();
        pad.as_pad3d_mut().round_way = -1;
        pad.as_pad3d_mut().lturn180 = 1;
        assert_eq!(pad.as_pad3d().round_way, -1);
        assert_eq!(pad.as_pad3d().lturn180, 1);

        pad.as_pad2d_mut().pow = 255;
        pad.as_pad2d_mut().dir = 1.57;
        assert_eq!(pad.as_pad2d().pow, 255);
        assert!((pad.as_pad2d().dir - 1.57).abs() < 1e-6);
    }
}
