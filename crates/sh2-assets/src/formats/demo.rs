//! Safe binary parser and state evaluator for Silent Hill 2 Cutscene Scripts (`.dds`, `Event/demoview.c`).
//!
//! # Ground Truth & Format Architecture
//! Verified against:
//! - **Primary Ground Truth**: PS2 Debug Prototype ELF (`SLUS_202.28`, DWARF compile unit `Event/demoview.c`:
//!   `DdsReadShort` `0x1b0d80`, `DdsReadFloat2` `0x1b0dc0`, `DdsReadFloat4` `0x1b0e40`, `DdsPlayKey` `0x1b1140`,
//!   `DdsPlayCharacter` `0x1b1220`, `DdsObjectRotation` `0x1b1340`, `DdsPlayCamera` `0x1b1720`,
//!   `DdsPlayLight` `0x1b1900`, `DramaDemoSkipLast` `0x1b1be0`, `DramaDemoInit` `0x1b2080`, `DdsPlay` `0x1b2520`).
//! - **Secondary Ground Truth**: Retail PC binary (`sh2pc.exe` NA v1.0: `0x49fbb0..0x4a2670`) and all
//!   130 retail `.dds` cutscene scripts across `data/demo/` and `data/demo2/`.
//!
//! ### `.dds` Header Layout (`25 + 16 * n_chars` bytes)
//! | Offset | Type | Field | Description |
//! |--------|------|-------|-------------|
//! | `0x00` | `[u8; 4]` | `magic` | `"dds\0"` (`[0x64, 0x64, 0x73, 0x00]`) |
//! | `0x04` | `f32` | `version_a` | `0.61f` (`f6 28 1c 3f`) |
//! | `0x08` | `f32` | `version_b` | `0.61f` (`f6 28 1c 3f`) |
//! | `0x0C` | `u32` | `reserved_0c` | `0` |
//! | `0x10` | `i16` | `total_frames` | Total duration in 30 FPS frames |
//! | `0x12` | `i16` | `total_objects` | Total animated objects in scene |
//! | `0x14` | `u8` | `point_lights` | Number of character point/omni lights |
//! | `0x15` | `u8` | `spot_lights` | Number of spot lights |
//! | `0x16` | `u8` | `inf_lights` | Number of parallel/directional lights |
//! | `0x17` | `u8` | `pad_17` | Reserved (`0`) |
//! | `0x18` | `u8` | `n_characters` | Number of actor/prop nodes (`0..=7`) |
//! | `0x19` | `[u8; 16] * n_characters` | `char_names` | 16-byte null-padded ASCII actor node names |

mod evaluator;

pub use evaluator::{DemoDdsEvaluator, DemoKeyframeState, DemoLightState};

use super::binary::{read_f32, read_i16, read_u16, read_u32};
use crate::error::AssetError;

/// `.dds` cutscene script header magic (`"dds\0"`).
pub const DEMO_DDS_MAGIC: [u8; 4] = *b"dds\0";

/// Maximum number of actor/prop nodes in a cutscene (`DEMO_MAX_CHARS = 8`).
pub const DEMO_MAX_CHARS: usize = 8;

/// Maximum number of dynamic lights in a cutscene (`DEMO_MAX_LIGHTS = 16`, `piano_lau.dds` has 16).
pub const DEMO_MAX_LIGHTS: usize = 16;

/// PS2 vertical screen height (`448.0`) used to convert projection distance `scr_z` to vertical FOV.
pub const DEMO_SCREEN_SY: f32 = 448.0;

/// Retail `DdsPlay` camera projection scale factor (`0x6b06ac` in `sh2pc.exe` = `1.14702`).
pub const DEMO_SCR_Z_SCALE: f32 = 1.147_02;

/// Unpacks a custom 16-bit half-float (`u16` little-endian) into an IEEE-754 `f32`,
/// matching `DdsReadFloat2` (`0x1b0dc0` in `SLUS_202.28`, `0x49fbe0` in `sh2pc.exe`).
///
/// Note: Unlike IEEE-754 binary16, `DdsReadFloat2` unconditionally adds `0x70` (`112`) to the
/// 5-bit exponent without a zero check, so `0x0000` evaluates to `2^-14 = 0.00006103515625`.
#[inline]
pub fn dds_f16_to_f32(h: u16) -> f32 {
    let h32 = h as u32;
    let sign = (h32 & 0x8000) << 16;
    let exp = (((h32 >> 10) & 0x1F).wrapping_add(0x70) & 0xFF) << 23;
    let mant = (h32 & 0x03FF) << 13;
    f32::from_bits(sign | exp | mant)
}

/// Converts Euler angles `(rx, ry, rz)` and `position` into a look-at `interest` target,
/// matching `DdsObjectRotation` (`0x1b1340` in `SLUS_202.28`, `0x4a01c0` in `sh2pc.exe`).
#[inline]
pub fn dds_rotation_to_interest(pos: [f32; 3], rot: [f32; 3]) -> [f32; 3] {
    let (sx, cx) = rot[0].sin_cos();
    let (sy, cy) = rot[1].sin_cos();
    [
        pos[0] + sy * cx * 1000.0,
        pos[1] - sx * 1000.0,
        pos[2] + cy * cx * 1000.0,
    ]
}

/// Light type classification inside a `.dds` cutscene script (`DdsPlay` light slots).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DemoLightKind {
    /// Character point / omni light with distance decay ramp (`0 .. point_lights`).
    Point,
    /// Spot light with cone angle and distance decay ramp (`point_lights .. point_lights + spot_lights`).
    Spot,
    /// Infinite / directional parallel light (`point_lights + spot_lights .. total_lights`).
    Directional,
}

/// Parsed header of a `.dds` cutscene script file.
#[derive(Debug, Clone, PartialEq)]
pub struct DemoDdsHeader {
    /// Exporter version float A (`0.61`).
    pub version_a: f32,
    /// Exporter version float B (`0.61`).
    pub version_b: f32,
    /// Total cutscene length in 30 FPS frames (`offset 0x10`).
    pub total_frames: u32,
    /// Total animated object count declared at `offset 0x12`.
    pub total_objects: i16,
    /// Count of character point/omni lights (`offset 0x14`).
    pub point_light_count: u8,
    /// Count of spot lights (`offset 0x15`).
    pub spot_light_count: u8,
    /// Count of directional/parallel lights (`offset 0x16`).
    pub inf_light_count: u8,
    /// Actor and prop node names (`offset 0x19..`, e.g. `"hhh_jms_pos"`, `"agl_pos"`).
    pub character_names: Vec<String>,
}

impl DemoDdsHeader {
    /// Total number of dynamic lights (`point_light_count + spot_light_count + inf_light_count`).
    #[inline]
    pub fn total_lights(&self) -> usize {
        (self.point_light_count as usize)
            + (self.spot_light_count as usize)
            + (self.inf_light_count as usize)
    }

    /// Returns the [`DemoLightKind`] for light slot `light_index` (`0..total_lights()`).
    #[inline]
    pub fn light_kind(&self, light_index: usize) -> DemoLightKind {
        let p = self.point_light_count as usize;
        let s = self.spot_light_count as usize;
        if light_index < p {
            DemoLightKind::Point
        } else if light_index < p + s {
            DemoLightKind::Spot
        } else {
            DemoLightKind::Directional
        }
    }
}

/// Camera operations inside Block `0x01` (`DdsPlayCamera`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DemoCameraOp {
    /// Sub-key `0x03`: Camera eye position (`is_raw == true` -> absolute `f32[3]`; `false` -> `f16[3]` delta from base).
    Position { values: [f32; 3], is_raw: bool },
    /// Sub-key `0x04`: Camera look-at interest (`is_raw == true` -> absolute `f32[3]`; `false` -> `f16[3]` delta from base).
    Interest { values: [f32; 3], is_raw: bool },
    /// Sub-key `0x05`: Camera Euler rotation `f16[3]` converted to interest + roll.
    Rotation([f32; 3]),
    /// Sub-key `0x06`: Camera roll angle in radians (`f16`).
    Roll(f32),
    /// Sub-key `0x07`: Camera projection focal plane distance (`f32`).
    Plane(f32),
}

/// Dynamic light operations inside Blocks `2 .. 2 + total_lights` (`DdsPlayLight`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DemoLightOp {
    /// Sub-keys `0x01` / `0x02`: Turn light on (`true`) or off (`false`).
    Visible(bool),
    /// Sub-key `0x03`: Light world position (`is_raw == true` -> absolute `f32[3]`; `false` -> `f16[3]` delta from base).
    Position { values: [f32; 3], is_raw: bool },
    /// Sub-key `0x04`: Light target interest (`is_raw == true` -> absolute `f32[3]`; `false` -> `f16[3]` delta from base).
    Interest { values: [f32; 3], is_raw: bool },
    /// Sub-key `0x05`: Light Euler orientation `f16[3]`.
    Rotation([f32; 3]),
    /// Sub-key `0x08`: Light RGB color (`f16[3]`).
    Color([f32; 3]),
    /// Sub-key `0x09`: Distance attenuation decay ramp `[start, end]` (`f16[2]`).
    Decay([f32; 2]),
    /// Sub-key `0x0A`: Spot light cone angle parameters `[inner, outer]` (`f16[2]`).
    Cone([f32; 2]),
}

/// Character / prop operations inside Blocks `>= 2 + total_lights` (`DdsPlayCharacter`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DemoCharOp {
    /// Sub-keys `0x01` / `0x02`: Show (`true`) or hide (`false`) character.
    Visible(bool),
    /// Sub-key `0x03`: Character world root position (`is_raw == true` -> absolute `f32[3]`; `false` -> `f16[3]` delta from base).
    Position { values: [f32; 3], is_raw: bool },
}

/// A single timestamped frame record in a `.dds` cutscene script.
#[derive(Debug, Clone, PartialEq)]
pub struct DemoFrameRecord {
    /// 30 FPS frame number (`0..total_frames`).
    pub frame: u16,
    /// Control key `0x10`: Hard camera cut (no interpolation into this frame; coordinates are raw `f32` that reset `base`).
    pub is_cut: bool,
    /// Control key `0x11`: Triggers the next set of character animations (`DramaDemoAnimationStart`).
    pub trigger_animations: bool,
    /// Control key `0x14`: Auxiliary flag present in some scripts.
    pub flag_14: bool,
    /// Camera operations in this frame record.
    pub camera_ops: Vec<DemoCameraOp>,
    /// Light operations `(light_index, op)` in this frame record.
    pub light_ops: Vec<(usize, DemoLightOp)>,
    /// Character operations `(char_index, op)` in this frame record.
    pub char_ops: Vec<(usize, DemoCharOp)>,
}

/// Complete parsed `.dds` cutscene script file.
#[derive(Debug, Clone, PartialEq)]
pub struct DemoDdsFile {
    /// Parsed `.dds` header metadata.
    pub header: DemoDdsHeader,
    /// Ordered frame keyframe records.
    pub records: Vec<DemoFrameRecord>,
    /// Frame numbers at which Control Key `0x11` (`DramaDemoAnimationStart`) fires.
    pub animation_trigger_frames: Vec<u16>,
}

/// Parses a Silent Hill 2 `.dds` cutscene script from raw bytes (`Event/demoview.c`).
pub fn parse_demo_dds(data: &[u8]) -> Result<DemoDdsFile, AssetError> {
    if data.len() < 25 {
        return Err(AssetError::BufferUnderflow {
            required: 25,
            available: data.len(),
        });
    }
    if data[0..3] != DEMO_DDS_MAGIC[0..3] {
        return Err(AssetError::InvalidMagic {
            expected: &DEMO_DDS_MAGIC,
            found: data[0..4].to_vec(),
        });
    }

    let version_a = read_f32(data, 0x04)?;
    let version_b = read_f32(data, 0x08)?;
    let _reserved = read_u32(data, 0x0C)?;
    let total_frames = read_i16(data, 0x10)?.max(0) as u32;
    let total_objects = read_i16(data, 0x12)?;
    let point_light_count = data[0x14];
    let spot_light_count = data[0x15];
    let inf_light_count = data[0x16];
    let n_chars = data[0x18] as usize;

    let mut p = 0x19usize;
    let names_end = p.saturating_add(n_chars * 16);
    if names_end > data.len() {
        return Err(AssetError::BufferUnderflow {
            required: names_end,
            available: data.len(),
        });
    }

    let mut character_names = Vec::with_capacity(n_chars);
    for _ in 0..n_chars {
        let raw_slice = &data[p..p + 16];
        let len = raw_slice
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(raw_slice.len());
        let name = String::from_utf8_lossy(&raw_slice[..len]).into_owned();
        character_names.push(name);
        p += 16;
    }

    let header = DemoDdsHeader {
        version_a,
        version_b,
        total_frames,
        total_objects,
        point_light_count,
        spot_light_count,
        inf_light_count,
        character_names,
    };

    let total_lights = header.total_lights();
    let mut records = Vec::new();
    let mut animation_trigger_frames = Vec::new();

    while p + 2 <= data.len() {
        let frame = read_u16(data, p)?;
        if frame == 0xFFFF {
            break;
        }
        if p + 3 > data.len() {
            return Err(AssetError::BufferUnderflow {
                required: p + 3,
                available: data.len(),
            });
        }
        let mut obj = data[p + 2];
        p += 3;

        let mut is_cut = false;
        let mut trigger_animations = false;
        let mut flag_14 = false;
        let mut camera_ops = Vec::new();
        let mut light_ops = Vec::new();
        let mut char_ops = Vec::new();

        while obj != 0xFF {
            if obj == 0 {
                loop {
                    if p >= data.len() {
                        return Err(AssetError::BufferUnderflow {
                            required: p + 1,
                            available: data.len(),
                        });
                    }
                    let key = data[p];
                    p += 1;
                    if key == 0x0B || !(0x0C..=0x14).contains(&key) {
                        break;
                    }
                    match key {
                        0x10 => is_cut = true,
                        0x11 => {
                            trigger_animations = true;
                            animation_trigger_frames.push(frame);
                        }
                        0x14 => flag_14 = true,
                        _ => {}
                    }
                }
            } else if obj == 1 {
                loop {
                    if p >= data.len() {
                        return Err(AssetError::BufferUnderflow {
                            required: p + 1,
                            available: data.len(),
                        });
                    }
                    let c = data[p];
                    p += 1;
                    match c {
                        3 => {
                            let values = read_vec3(data, &mut p, is_cut)?;
                            camera_ops.push(DemoCameraOp::Position {
                                values,
                                is_raw: is_cut,
                            });
                        }
                        4 => {
                            let values = read_vec3(data, &mut p, is_cut)?;
                            camera_ops.push(DemoCameraOp::Interest {
                                values,
                                is_raw: is_cut,
                            });
                        }
                        5 => {
                            let r = read_vec3_f16(data, &mut p)?;
                            camera_ops.push(DemoCameraOp::Rotation(r));
                        }
                        6 => {
                            let roll = dds_f16_to_f32(read_u16(data, p)?);
                            p += 2;
                            camera_ops.push(DemoCameraOp::Roll(roll));
                        }
                        7 => {
                            let plane = read_f32(data, p)?;
                            p += 4;
                            camera_ops.push(DemoCameraOp::Plane(plane));
                        }
                        _ => break,
                    }
                }
            } else if (obj as usize) >= 2 && (obj as usize) < 2 + total_lights {
                let light_idx = (obj as usize) - 2;
                loop {
                    if p >= data.len() {
                        return Err(AssetError::BufferUnderflow {
                            required: p + 1,
                            available: data.len(),
                        });
                    }
                    let c = data[p];
                    p += 1;
                    match c {
                        1 => light_ops.push((light_idx, DemoLightOp::Visible(true))),
                        2 => light_ops.push((light_idx, DemoLightOp::Visible(false))),
                        3 => {
                            let values = read_vec3(data, &mut p, is_cut)?;
                            light_ops.push((
                                light_idx,
                                DemoLightOp::Position {
                                    values,
                                    is_raw: is_cut,
                                },
                            ));
                        }
                        4 => {
                            let values = read_vec3(data, &mut p, is_cut)?;
                            light_ops.push((
                                light_idx,
                                DemoLightOp::Interest {
                                    values,
                                    is_raw: is_cut,
                                },
                            ));
                        }
                        5 => {
                            let r = read_vec3_f16(data, &mut p)?;
                            light_ops.push((light_idx, DemoLightOp::Rotation(r)));
                        }
                        8 => {
                            let col = read_vec3_f16(data, &mut p)?;
                            light_ops.push((light_idx, DemoLightOp::Color(col)));
                        }
                        9 => {
                            let d0 = dds_f16_to_f32(read_u16(data, p)?);
                            let d1 = dds_f16_to_f32(read_u16(data, p + 2)?);
                            p += 4;
                            light_ops.push((light_idx, DemoLightOp::Decay([d0, d1])));
                        }
                        10 => {
                            let c0 = dds_f16_to_f32(read_u16(data, p)?);
                            let c1 = dds_f16_to_f32(read_u16(data, p + 2)?);
                            p += 4;
                            light_ops.push((light_idx, DemoLightOp::Cone([c0, c1])));
                        }
                        _ => break,
                    }
                }
            } else if (obj as usize) >= 2 + total_lights
                && (obj as usize) < 2 + total_lights + n_chars
            {
                let char_idx = (obj as usize) - 2 - total_lights;
                loop {
                    if p >= data.len() {
                        return Err(AssetError::BufferUnderflow {
                            required: p + 1,
                            available: data.len(),
                        });
                    }
                    let c = data[p];
                    p += 1;
                    match c {
                        1 => char_ops.push((char_idx, DemoCharOp::Visible(true))),
                        2 => char_ops.push((char_idx, DemoCharOp::Visible(false))),
                        3 => {
                            let values = read_vec3(data, &mut p, is_cut)?;
                            char_ops.push((
                                char_idx,
                                DemoCharOp::Position {
                                    values,
                                    is_raw: is_cut,
                                },
                            ));
                        }
                        _ => break,
                    }
                }
            } else {
                return Err(AssetError::InvalidData(format!(
                    "Invalid .dds object code {obj:#x} at offset {p:#x}"
                )));
            }

            if p >= data.len() {
                return Err(AssetError::BufferUnderflow {
                    required: p + 1,
                    available: data.len(),
                });
            }
            obj = data[p];
            p += 1;
        }

        records.push(DemoFrameRecord {
            frame,
            is_cut,
            trigger_animations,
            flag_14,
            camera_ops,
            light_ops,
            char_ops,
        });
    }

    Ok(DemoDdsFile {
        header,
        records,
        animation_trigger_frames,
    })
}

#[inline]
fn read_vec3_f16(data: &[u8], p: &mut usize) -> Result<[f32; 3], AssetError> {
    if *p + 6 > data.len() {
        return Err(AssetError::BufferUnderflow {
            required: *p + 6,
            available: data.len(),
        });
    }
    let x = dds_f16_to_f32(read_u16(data, *p)?);
    let y = dds_f16_to_f32(read_u16(data, *p + 2)?);
    let z = dds_f16_to_f32(read_u16(data, *p + 4)?);
    *p += 6;
    Ok([x, y, z])
}

#[inline]
fn read_vec3(data: &[u8], p: &mut usize, is_raw: bool) -> Result<[f32; 3], AssetError> {
    if is_raw {
        if *p + 12 > data.len() {
            return Err(AssetError::BufferUnderflow {
                required: *p + 12,
                available: data.len(),
            });
        }
        let x = read_f32(data, *p)?;
        let y = read_f32(data, *p + 4)?;
        let z = read_f32(data, *p + 8)?;
        *p += 12;
        Ok([x, y, z])
    } else {
        read_vec3_f16(data, p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dds_f16_to_f32_ground_truth() {
        assert_eq!(dds_f16_to_f32(0x0000).to_bits(), 0x3800_0000);
        assert_eq!(dds_f16_to_f32(0x3C00), 1.0);
        assert_eq!(dds_f16_to_f32(0xC000), -2.0);
    }

    #[test]
    fn test_parse_and_evaluate_synthetic_demo_dds() {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"dds\0");
        buf.extend_from_slice(&0.61f32.to_le_bytes());
        buf.extend_from_slice(&0.61f32.to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.extend_from_slice(&60i16.to_le_bytes());
        buf.extend_from_slice(&3i16.to_le_bytes());
        buf.push(1);
        buf.push(0);
        buf.push(0);
        buf.push(0);
        buf.push(1);
        let mut cname = [0u8; 16];
        cname[..11].copy_from_slice(b"hhh_jms_pos");
        buf.extend_from_slice(&cname);

        buf.extend_from_slice(&0u16.to_le_bytes());
        buf.push(0);
        buf.push(0x10);
        buf.push(0x11);
        buf.push(0x0B);
        buf.push(1);
        buf.push(3);
        buf.extend_from_slice(&100.0f32.to_le_bytes());
        buf.extend_from_slice(&(-1500.0f32).to_le_bytes());
        buf.extend_from_slice(&200.0f32.to_le_bytes());
        buf.push(4);
        buf.extend_from_slice(&0.0f32.to_le_bytes());
        buf.extend_from_slice(&(-1400.0f32).to_le_bytes());
        buf.extend_from_slice(&0.0f32.to_le_bytes());
        buf.push(7);
        buf.extend_from_slice(&320.0f32.to_le_bytes());
        buf.push(0x0B);
        buf.push(3);
        buf.push(1);
        buf.push(3);
        buf.extend_from_slice(&10.0f32.to_le_bytes());
        buf.extend_from_slice(&0.0f32.to_le_bytes());
        buf.extend_from_slice(&20.0f32.to_le_bytes());
        buf.push(0x0B);
        buf.push(0xFF);

        buf.extend_from_slice(&1u16.to_le_bytes());
        buf.push(1);
        buf.push(3);
        buf.extend_from_slice(&0x3C00u16.to_le_bytes());
        buf.extend_from_slice(&0x3C00u16.to_le_bytes());
        buf.extend_from_slice(&0x3C00u16.to_le_bytes());
        buf.push(0x0B);
        buf.push(0xFF);

        buf.extend_from_slice(&0xFFFFu16.to_le_bytes());

        let parsed = parse_demo_dds(&buf).expect("valid synthetic .dds");
        assert_eq!(parsed.header.total_frames, 60);
        assert_eq!(parsed.header.character_names, vec!["hhh_jms_pos"]);
        assert_eq!(parsed.animation_trigger_frames, vec![0]);

        let mut eval = DemoDdsEvaluator::new(parsed);
        let (st, _) = eval.sample_at(0.5, 0.0, 0.0);
        assert!((st.camera_pos[0] - 100.5).abs() < 1e-3);
        assert!(st.char_visible[0]);
        assert!((st.char_positions[0][0] - 10.0).abs() < 1e-3);
    }
}
