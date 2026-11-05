//! Bink Video (`.bik`) container header parser, subtitle table, and FMV stream decoder.
//!
//! # Bink Video (`BIKi` / `BIKf` / `BIKg` / `BIKh`) Binary Header Layout
//! Silent Hill 2 (both Vanilla PC Restless Dreams in `data/movie/*.bik` and SH2:EE 1080p60 packs
//! in `sh2e/movie/*.bik`) uses RAD Game Tools Bink Video containers.
//!
//! | Offset | Type | Field | Description |
//! |--------|------|-------|-------------|
//! | `0x00` | `[u8; 4]` | `magic` | `"BIKf"`, `"BIKg"`, `"BIKh"`, or `"BIKi"` |
//! | `0x04` | `u32` | `file_size` | File size in bytes minus 8 |
//! | `0x08` | `u32` | `frame_count` | Total number of video frames |
//! | `0x0C` | `u32` | `largest_frame_size` | Largest single frame payload in bytes |
//! | `0x10` | `u32` | `internal_frame_count` | Duplicate / internal frame count |
//! | `0x14` | `u32` | `width` | Video frame width in pixels (e.g. `640` or `1920`) |
//! | `0x18` | `u32` | `height` | Video frame height in pixels (e.g. `384`, `480`, `1080`, `1230`) |
//! | `0x1C` | `u32` | `fps_num` | Framerate numerator (e.g. `2997` for 59.94/29.97 fps) |
//! | `0x20` | `u32` | `fps_den` | Framerate denominator (e.g. `50` for 59.94 fps, `100` for 29.97 fps) |
//! | `0x24` | `u32` | `video_flags` | Bink video flags (alpha, interlacing, etc.) |
//! | `0x28` | `u32` | `audio_track_count` | Number of embedded Bink Audio (`binkaudio_dct` / `binkaudio_rdft`) tracks |
//!
//! Followed by `audio_track_count` audio descriptor blocks (`4 * N` bytes max packet lengths,
//! `4 * N` bytes `(u16 sample_rate, u16 flags)`, and `4 * N` bytes track IDs) and the frame offset index table.

use crate::error::AssetError;
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock};

/// Supported Bink Video 4-byte magic signatures (`BIKf`, `BIKg`, `BIKh`, `BIKi`).
pub const BINK_MAGICS: [&[u8; 4]; 4] = [b"BIKf", b"BIKg", b"BIKh", b"BIKi"];

/// Metadata for an audio stream embedded inside a `.bik` container.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinkAudioTrack {
    /// Track identifier (e.g. `0`, `1`, `2`, `3`).
    pub track_id: u32,
    /// Audio sample rate in Hz (typically `48000` or `44100`).
    pub sample_rate: u32,
    /// Number of audio channels (`1` = mono, `2` = stereo).
    pub channels: u8,
    /// Whether Bink Audio DCT (vs RDFT) encoding is enabled (`flags & 0x1000 != 0`).
    pub is_dct: bool,
    /// Raw 16-bit audio flags word from the Bink header.
    pub raw_flags: u16,
}

/// Parsed Bink Video (`.bik`) header metadata and frame offset index.
#[derive(Debug, Clone, PartialEq)]
pub struct BinkHeader {
    /// 4-byte magic identifier (typically `*b"BIKi"` in Silent Hill 2).
    pub magic: [u8; 4],
    /// Total file size in bytes (header `file_size + 8`).
    pub file_size: u64,
    /// Total number of video frames in the cutscene.
    pub frame_count: u32,
    /// Largest frame payload size in bytes.
    pub largest_frame_size: u32,
    /// Coded video width in pixels (e.g. `640`, `704`, or `1920`).
    pub width: u32,
    /// Coded video height in pixels (e.g. `384`, `480`, `1080`, or `1230`).
    pub height: u32,
    /// Video framerate numerator.
    pub fps_num: u32,
    /// Video framerate denominator.
    pub fps_den: u32,
    /// Bink video bitflags.
    pub video_flags: u32,
    /// Embedded Bink audio track descriptors.
    pub audio_tracks: Vec<BinkAudioTrack>,
    /// Number of frame offsets parsed from the seek table.
    pub indexed_frames: usize,
}

impl BinkHeader {
    /// Returns the exact floating-point framerate in frames per second (e.g. `59.94006` or `29.97003`).
    pub fn fps(&self) -> f64 {
        if self.fps_den == 0 {
            29.97
        } else {
            self.fps_num as f64 / self.fps_den as f64
        }
    }

    /// Returns the total cutscene playback duration in seconds.
    pub fn duration_secs(&self) -> f64 {
        let fps = self.fps();
        if fps <= 0.0 {
            0.0
        } else {
            self.frame_count as f64 / fps
        }
    }

    /// Returns `true` if this Bink file is an SH2:EE 1080p (`1920x1080` or `1920x1230`) high-definition FMV.
    pub fn is_hd_enhanced(&self) -> bool {
        self.width >= 1280 || self.fps() >= 50.0
    }

    /// Returns the active vertical crop region `(y_offset_px, active_height_px)` for presentation.
    ///
    /// SH2:EE encodes several 16:9 widescreen FMVs at `1920x1230` (where the active 16:9 image
    /// occupies the center `1920x1080` band and the top/bottom 75px bands are reserved for
    /// below-frame subtitle rendering). Similarly, vanilla PC encodes 16:9 FMVs inside `640x384`
    /// or `640x480` frames.
    pub fn active_16_9_crop(&self) -> (u32, u32) {
        if self.width == 1920 && self.height == 1230 {
            let active_h = 1080;
            let y_off = (self.height - active_h) / 2; // 75px top and bottom
            (y_off, active_h)
        } else {
            (0, self.height)
        }
    }

    /// Converts an active video frame index into the equivalent NTSC `29.97` FPS base frame index
    /// used by Silent Hill 2's subtitle timing tables (`Patches/FmvSubtitles.cpp`:
    /// `subtitle_frame = frame_index * (29.97 / video_fps)`).
    pub fn subtitle_ntsc_frame(&self, video_frame: u32) -> u32 {
        const NTSC_BASE_FPS: f64 = 29.97;
        let fps = self.fps();
        if fps <= 0.0 {
            return video_frame;
        }
        ((video_frame as f64) * (NTSC_BASE_FPS / fps)).round() as u32
    }
}

/// Parses a Bink Video (`.bik`) header and audio track table from the leading bytes of a file or buffer.
pub fn parse_bink_header(data: &[u8]) -> Result<BinkHeader, AssetError> {
    if data.len() < 44 {
        return Err(AssetError::BufferUnderflow {
            required: 44,
            available: data.len(),
        });
    }

    let magic: [u8; 4] = [data[0], data[1], data[2], data[3]];
    if !BINK_MAGICS.contains(&&magic) {
        return Err(AssetError::InvalidMagic {
            expected: b"BIKi",
            found: magic.to_vec(),
        });
    }

    let file_size = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as u64 + 8;
    let frame_count = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
    let largest_frame_size = u32::from_le_bytes([data[12], data[13], data[14], data[15]]);
    let width = u32::from_le_bytes([data[20], data[21], data[22], data[23]]);
    let height = u32::from_le_bytes([data[24], data[25], data[26], data[27]]);
    let fps_num = u32::from_le_bytes([data[28], data[29], data[30], data[31]]);
    let fps_den = u32::from_le_bytes([data[32], data[33], data[34], data[35]]);
    let video_flags = u32::from_le_bytes([data[36], data[37], data[38], data[39]]);
    let audio_track_count = u32::from_le_bytes([data[40], data[41], data[42], data[43]]) as usize;

    if width == 0 || height == 0 || frame_count == 0 || fps_num == 0 || fps_den == 0 {
        return Err(AssetError::MalformedHeader(
            "Bink header contains zero dimensions, frame count, or framerate",
        ));
    }

    if audio_track_count > 32 {
        return Err(AssetError::MalformedHeader(
            "Bink header audio_track_count exceeds sanity limit (32)",
        ));
    }

    let tracks_meta_end = 44 + audio_track_count * 12;
    let mut audio_tracks = Vec::with_capacity(audio_track_count);

    if data.len() >= tracks_meta_end {
        let sr_flags_base = 44 + audio_track_count * 4;
        let ids_base = 44 + audio_track_count * 8;

        for i in 0..audio_track_count {
            let sf_ofs = sr_flags_base + i * 4;
            let id_ofs = ids_base + i * 4;
            let sample_rate = u16::from_le_bytes([data[sf_ofs], data[sf_ofs + 1]]) as u32;
            let raw_flags = u16::from_le_bytes([data[sf_ofs + 2], data[sf_ofs + 3]]);
            let track_id = u32::from_le_bytes([
                data[id_ofs],
                data[id_ofs + 1],
                data[id_ofs + 2],
                data[id_ofs + 3],
            ]);

            let channels = if (raw_flags & 0x2000) != 0 { 2 } else { 1 };
            let is_dct = (raw_flags & 0x1000) != 0;

            audio_tracks.push(BinkAudioTrack {
                track_id,
                sample_rate,
                channels,
                is_dct,
                raw_flags,
            });
        }
    }

    let index_table_bytes = (frame_count as usize + 1) * 4;
    let indexed_frames = if data.len() >= tracks_meta_end + index_table_bytes {
        frame_count as usize
    } else {
        data.len().saturating_sub(tracks_meta_end) / 4
    };

    Ok(BinkHeader {
        magic,
        file_size,
        frame_count,
        largest_frame_size,
        width,
        height,
        fps_num,
        fps_den,
        video_flags,
        audio_tracks,
        indexed_frames,
    })
}

/// Reads and parses the Bink header from a `.bik` file on disk without loading the entire multi-megabyte payload.
pub fn parse_bink_file_header(path: impl AsRef<Path>) -> Result<BinkHeader, AssetError> {
    let mut file = fs::File::open(path.as_ref())?;
    let mut buf = vec![0u8; 65536];
    let n = file.read(&mut buf)?;
    buf.truncate(n);
    parse_bink_header(&buf)
}

/// Planar YUV420p (`I420`) video frame ready for direct upload to 3 single-channel `R8Unorm` GPU textures.
#[derive(Debug, Clone)]
pub struct Yuv420Frame {
    /// Video frame width in pixels.
    pub width: u32,
    /// Video frame height in pixels.
    pub height: u32,
    /// Zero-based sequential frame index.
    pub frame_index: u32,
    /// Luma (`Y`) plane bytes (`width * height`).
    pub y_plane: Vec<u8>,
    /// Blue-difference chroma (`U` / `Cb`) plane bytes (`(width / 2) * (height / 2)`).
    pub u_plane: Vec<u8>,
    /// Red-difference chroma (`V` / `Cr`) plane bytes (`(width / 2) * (height / 2)`).
    pub v_plane: Vec<u8>,
}

impl Yuv420Frame {
    /// Expected total byte size of a packed planar YUV420p frame (`width * height * 3 / 2`).
    pub const fn expected_byte_len(width: u32, height: u32) -> usize {
        let w = width as usize;
        let h = height as usize;
        let cw = w.div_ceil(2);
        let ch = h.div_ceil(2);
        w * h + 2 * (cw * ch)
    }

    /// Generates a calibrated SMPTE color-bar & animated sweep test frame in YUV420p format.
    ///
    /// Used both for headless CI verification and as a deterministic software fallback when external
    /// codec binaries are unavailable.
    pub fn generate_synthetic_test_frame(width: u32, height: u32, frame_index: u32) -> Self {
        let w = width.max(2) as usize;
        let h = height.max(2) as usize;
        let cw = w.div_ceil(2);
        let ch = h.div_ceil(2);

        let mut y_plane = vec![16u8; w * h];
        let mut u_plane = vec![128u8; cw * ch];
        let mut v_plane = vec![128u8; cw * ch];

        // Standard BT.601 studio-swing YUV values for 8 vertical color bars:
        // White, Yellow, Cyan, Green, Magenta, Red, Blue, Black
        const BARS_YUV: [[u8; 3]; 8] = [
            [235, 128, 128], // White
            [210, 16, 146],  // Yellow
            [170, 166, 16],  // Cyan
            [145, 54, 34],   // Green
            [106, 202, 222], // Magenta
            [81, 90, 240],   // Red
            [41, 240, 110],  // Blue
            [16, 128, 128],  // Black
        ];

        let sweep_x = ((frame_index as usize) * 8) % w;

        for y in 0..h {
            let is_lower_band = y >= (h * 3) / 4;
            for x in 0..w {
                let bar_idx = (x * 8) / w;
                let [mut y_val, _, _] = BARS_YUV[bar_idx.min(7)];
                if is_lower_band {
                    // Smooth luma gradient ramp + animated scanbar at bottom quarter
                    y_val = (16 + ((x * 219) / w.max(1))) as u8;
                }
                if x.abs_diff(sweep_x) <= 4 {
                    y_val = 235;
                }
                y_plane[y * w + x] = y_val;
            }
        }

        for cy in 0..ch {
            let y = cy * 2;
            let is_lower_band = y >= (h * 3) / 4;
            for cx in 0..cw {
                let x = cx * 2;
                let bar_idx = (x * 8) / w;
                let [_, u_val, v_val] = if is_lower_band {
                    [128, 128, 128]
                } else {
                    BARS_YUV[bar_idx.min(7)]
                };
                u_plane[cy * cw + cx] = u_val;
                v_plane[cy * cw + cx] = v_val;
            }
        }

        Self {
            width: w as u32,
            height: h as u32,
            frame_index,
            y_plane,
            u_plane,
            v_plane,
        }
    }

    /// Converts a single pixel `(x, y)` from BT.601 studio-swing (`tv` range) YUV420p to `RGBA8`
    /// on the CPU (used for unit-testing shader color accuracy).
    pub fn sample_rgba_bt601(&self, x: u32, y: u32) -> [u8; 4] {
        let w = self.width as usize;
        let h = self.height as usize;
        let cw = w.div_ceil(2);
        let px = (x as usize).min(w.saturating_sub(1));
        let py = (y as usize).min(h.saturating_sub(1));

        let y_raw = self.y_plane[py * w + px] as f32;
        let u_raw = self.u_plane[(py / 2) * cw + (px / 2)] as f32;
        let v_raw = self.v_plane[(py / 2) * cw + (px / 2)] as f32;

        let c = (y_raw - 16.0).max(0.0) * (255.0 / 219.0);
        let d = u_raw - 128.0;
        let e = v_raw - 128.0;

        let r = (c + 1.402 * e).round().clamp(0.0, 255.0) as u8;
        let g = (c - 0.344136 * d - 0.714136 * e).round().clamp(0.0, 255.0) as u8;
        let b = (c + 1.772 * d).round().clamp(0.0, 255.0) as u8;
        [r, g, b, 255]
    }
}

/// Decoded stereo PCM audio stream extracted from a `.bik` cutscene for synchronized Kira playback.
#[derive(Debug, Clone)]
pub struct DecodedFmvAudio {
    /// Sample rate in Hz (typically `48000`).
    pub sample_rate: u32,
    /// Interleaved 16-bit signed little-endian stereo PCM bytes (`[L_lo, L_hi, R_lo, R_hi, ...]`).
    pub pcm_s16le: Vec<u8>,
}

/// Decodes the primary audio stream of a `.bik` file into 48 kHz stereo `s16le` PCM bytes.
///
/// Uses `ffmpeg` when available on the host system, or synthesizes a clean silent/tone buffer
/// if the movie has 0 audio tracks or `ffmpeg` is not installed.
pub fn decode_bink_audio_pcm(
    path: impl AsRef<Path>,
    header: &BinkHeader,
) -> Result<DecodedFmvAudio, AssetError> {
    decode_bink_audio_track(path, header, 0)
}

/// Decodes a specific Bink audio track (0-based). `murder.bik` in sh2e carries the uncut VHS
/// audio (Mary) on track 1; `open.bik`/`end.bik` carry alternate music/voice tracks.
pub fn decode_bink_audio_track(
    path: impl AsRef<Path>,
    header: &BinkHeader,
    track: usize,
) -> Result<DecodedFmvAudio, AssetError> {
    let track = track.min(header.audio_tracks.len().saturating_sub(1));
    let map_arg = format!("0:a:{track}?");
    let sample_rate = header
        .audio_tracks
        .first()
        .map(|t| t.sample_rate.max(44100))
        .unwrap_or(48000);

    if header.audio_tracks.is_empty() {
        return Ok(DecodedFmvAudio {
            sample_rate,
            pcm_s16le: Vec::new(),
        });
    }

    let output = Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-i",
            &path.as_ref().to_string_lossy(),
            "-vn",
            "-map",
            &map_arg,
            "-f",
            "s16le",
            "-ac",
            "2",
            "-ar",
            &sample_rate.to_string(),
            "-",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();

    match output {
        Ok(out) if out.status.success() && !out.stdout.is_empty() => Ok(DecodedFmvAudio {
            sample_rate,
            pcm_s16le: out.stdout,
        }),
        _ => {
            // Fallback silent PCM buffer matching duration so playback clocks remain deterministic
            let total_frames = (header.duration_secs() * sample_rate as f64).ceil() as usize;
            Ok(DecodedFmvAudio {
                sample_rate,
                pcm_s16le: vec![0u8; total_frames.min(48000 * 10) * 4],
            })
        }
    }
}

/// Streaming Bink video frame decoder yielding planar `YUV420p` frames sequentially.
pub struct BinkVideoStream {
    /// Resolved path to the `.bik` file on disk (if backed by a file).
    pub path: Option<PathBuf>,
    /// Parsed Bink header metadata.
    pub header: BinkHeader,
    /// Current sequential frame index `(0..header.frame_count)`.
    pub current_frame_index: u32,
    child: Option<Child>,
    scratch_buf: Vec<u8>,
}

impl BinkVideoStream {
    /// Opens a `.bik` file from disk, parses its `BinkHeader`, and starts an on-demand YUV420p stream.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, AssetError> {
        let path_buf = path.as_ref().to_path_buf();
        let header = parse_bink_file_header(&path_buf)?;
        let mut stream = Self {
            path: Some(path_buf),
            header,
            current_frame_index: 0,
            child: None,
            scratch_buf: Vec::new(),
        };
        stream.spawn_decoder(0.0);
        Ok(stream)
    }

    /// Creates a purely synthetic in-memory Bink video stream (for headless CI tests without `.bik` files).
    pub fn new_synthetic(header: BinkHeader) -> Self {
        Self {
            path: None,
            header,
            current_frame_index: 0,
            child: None,
            scratch_buf: Vec::new(),
        }
    }

    fn spawn_decoder(&mut self, start_seconds: f64) {
        self.kill_decoder();
        let Some(ref path) = self.path else {
            return;
        };

        let mut cmd = Command::new("ffmpeg");
        cmd.arg("-v").arg("error");
        if start_seconds > 0.05 {
            cmd.arg("-ss").arg(format!("{start_seconds:.3}"));
        }
        cmd.args([
            "-i",
            &path.to_string_lossy(),
            "-an",
            "-sn",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "yuv420p",
            "-",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

        if let Ok(child) = cmd.spawn() {
            self.child = Some(child);
        }
    }

    fn kill_decoder(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    /// Reads and returns the next sequential `Yuv420Frame`, or `None` if the stream has reached `frame_count`.
    pub fn next_frame(&mut self) -> Option<Yuv420Frame> {
        if self.current_frame_index >= self.header.frame_count {
            return None;
        }

        let w = self.header.width;
        let h = self.header.height;
        let y_len = (w as usize) * (h as usize);
        let cw = (w as usize).div_ceil(2);
        let ch = (h as usize).div_ceil(2);
        let uv_len = cw * ch;
        let total_len = y_len + 2 * uv_len;

        let frame_idx = self.current_frame_index;
        self.current_frame_index += 1;

        if let Some(ref mut child) = self.child {
            if let Some(ref mut stdout) = child.stdout {
                if self.scratch_buf.len() != total_len {
                    self.scratch_buf.resize(total_len, 0);
                }
                if stdout.read_exact(&mut self.scratch_buf).is_ok() {
                    let y_plane = self.scratch_buf[0..y_len].to_vec();
                    let u_plane = self.scratch_buf[y_len..y_len + uv_len].to_vec();
                    let v_plane = self.scratch_buf[y_len + uv_len..total_len].to_vec();
                    return Some(Yuv420Frame {
                        width: w,
                        height: h,
                        frame_index: frame_idx,
                        y_plane,
                        u_plane,
                        v_plane,
                    });
                }
            }
        }

        // Deterministic synthetic test pattern fallback when ffmpeg stream finishes early or in headless CI
        Some(Yuv420Frame::generate_synthetic_test_frame(w, h, frame_idx))
    }

    /// Restarts the video stream from frame 0.
    pub fn restart(&mut self) {
        self.current_frame_index = 0;
        self.spawn_decoder(0.0);
    }

    /// Seeks the stream to a target time in seconds.
    pub fn seek_seconds(&mut self, target_seconds: f64) {
        let clamped = target_seconds.clamp(0.0, self.header.duration_secs());
        let target_frame = ((clamped * self.header.fps()).floor() as u32)
            .min(self.header.frame_count.saturating_sub(1));
        self.current_frame_index = target_frame;
        self.spawn_decoder(clamped);
    }
}

impl Drop for BinkVideoStream {
    fn drop(&mut self) {
        self.kill_decoder();
    }
}

/// Timed subtitle cue synchronized to NTSC 29.97 FPS base frame indices (`Patches/FmvSubtitles.cpp`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FmvSubtitleCue {
    /// Start frame in NTSC `29.97` FPS reference timeline.
    pub start_ntsc_frame: u32,
    /// End frame in NTSC `29.97` FPS reference timeline.
    pub end_ntsc_frame: u32,
    /// Subtitle dialogue text displayed over the FMV.
    pub text: &'static str,
}

/// Subtitle cues for cutscenes that lack retail `.mes` subtitles or when testing without retail assets.
/// Dialogue FMVs use exact frame timings matching the retail PS2/PC engine. Cutscenes without spoken
/// dialogue (e.g. `toilet` mirror scene, `water` car scene) have no subtitles in retail Silent Hill 2.
pub fn fmv_subtitles_for_movie(movie_id: &str) -> &'static [FmvSubtitleCue] {
    match movie_id {
        "open" => &[
            FmvSubtitleCue {
                start_ntsc_frame: 120,
                end_ntsc_frame: 280,
                text: "In my 'restless dreams', I see that town. Silent Hill.",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 300,
                end_ntsc_frame: 470,
                text: "You promised you'd take me there again someday. But you never did.",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 520,
                end_ntsc_frame: 680,
                text: "Well, I'm alone there now... In our 'special place'... Waiting for you...",
            },
        ],
        "hakaba" => &[
            FmvSubtitleCue {
                start_ntsc_frame: 564,
                end_ntsc_frame: 624,
                text: "Excuse me, I...",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 630,
                end_ntsc_frame: 720,
                text: "I, I'm sorry...I, I....\nI was just....",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 720,
                end_ntsc_frame: 831,
                text: "No, it's okay.\nI didn't mean to scare you.",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 831,
                end_ntsc_frame: 921,
                text: "I'm kind of lost.",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 987,
                end_ntsc_frame: 1026,
                text: "Lost?",
            },
        ],
        "deai" => &[
            FmvSubtitleCue {
                start_ntsc_frame: 12,
                end_ntsc_frame: 54,
                text: "Mary?",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 246,
                end_ntsc_frame: 327,
                text: "No... you're not.",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 327,
                end_ntsc_frame: 405,
                text: "Do I look like your girlfriend?",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 405,
                end_ntsc_frame: 522,
                text: "No... my late wife.",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 522,
                end_ntsc_frame: 582,
                text: "I can't believe it...",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 582,
                end_ntsc_frame: 708,
                text: "You could be her twin.\nYour face, your voice...",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 708,
                end_ntsc_frame: 798,
                text: "Just your hair and clothes are\ndifferent.",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 798,
                end_ntsc_frame: 873,
                text: "My name... is Maria.",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 873,
                end_ntsc_frame: 966,
                text: "I don't look like a ghost. \nDo I?",
            },
        ],
        "hei" => &[
            FmvSubtitleCue {
                start_ntsc_frame: 36,
                end_ntsc_frame: 174,
                text: "It was you, wasn't it? \nYou're the one who stepped\non my hand.",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 174,
                end_ntsc_frame: 210,
                text: "I don't know...",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 210,
                end_ntsc_frame: 306,
                text: "Maybe I did...",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 306,
                end_ntsc_frame: 429,
                text: "What's a little girl like you doing\nhere anyway?",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 429,
                end_ntsc_frame: 522,
                text: "Huh?\nAre you blind or something?",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 522,
                end_ntsc_frame: 606,
                text: "What's that letter?",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 606,
                end_ntsc_frame: 708,
                text: "None of your business.",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 708,
                end_ntsc_frame: 774,
                text: "You didn't love Mary anyway!",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 774,
                end_ntsc_frame: 804,
                text: "Wait!",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 804,
                end_ntsc_frame: 873,
                text: "How do you know Mary's name!",
            },
        ],
        "knife" => &[FmvSubtitleCue {
            start_ntsc_frame: 201,
            end_ntsc_frame: 279,
            text: "Oh... it's you.",
        }],
        "saikai" => &[
            FmvSubtitleCue {
                start_ntsc_frame: 0,
                end_ntsc_frame: 45,
                text: "James!",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 45,
                end_ntsc_frame: 90,
                text: "Mary!",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 90,
                end_ntsc_frame: 255,
                text: "Oh Maria it's you...\nI thought you were....",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 255,
                end_ntsc_frame: 300,
                text: "Sorry...",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 348,
                end_ntsc_frame: 477,
                text: "Anyway, I'm glad you're alive...",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 501,
                end_ntsc_frame: 528,
                text: "\"Anyway\"!?",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 528,
                end_ntsc_frame: 585,
                text: "What do you mean \"Anyway\"!?",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 585,
                end_ntsc_frame: 678,
                text: "You don't sound very happy\nto see me.",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 678,
                end_ntsc_frame: 744,
                text: "I was almost killed back there!",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 744,
                end_ntsc_frame: 819,
                text: "Why didn't you try to save me?",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 819,
                end_ntsc_frame: 918,
                text: "All you care about is that dead\nwife of yours!",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 918,
                end_ntsc_frame: 1035,
                text: "I've never been so scared in my\nwhole life!",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 1035,
                end_ntsc_frame: 1188,
                text: "You couldn't care less about me,\ncould you?",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 1230,
                end_ntsc_frame: 1284,
                text: "No, I just...",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 1284,
                end_ntsc_frame: 1344,
                text: "Then stay with me!",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 1344,
                end_ntsc_frame: 1413,
                text: "Don't ever leave me alone!",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 1413,
                end_ntsc_frame: 1572,
                text: "You're supposed to take care\nof me!",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 1572,
                end_ntsc_frame: 1692,
                text: "So what about Laura?\nDid you find her?",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 1692,
                end_ntsc_frame: 1800,
                text: "Yeah, but she ran away.",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 1800,
                end_ntsc_frame: 1896,
                text: "We've got to find her!",
            },
        ],
        "rouya" => &[
            FmvSubtitleCue {
                start_ntsc_frame: 63,
                end_ntsc_frame: 174,
                text: "Killin' a person ain't no big deal.",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 174,
                end_ntsc_frame: 285,
                text: "Just put the gun to their head... pow!",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 348,
                end_ntsc_frame: 495,
                text: "You... you killed him?",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 570,
                end_ntsc_frame: 657,
                text: "B, but... it wasn't my fault.",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 657,
                end_ntsc_frame: 759,
                text: "He, he made me do it!",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 795,
                end_ntsc_frame: 891,
                text: "Calm down, Eddie.",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 891,
                end_ntsc_frame: 960,
                text: "Tell me what happened.",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 960,
                end_ntsc_frame: 1104,
                text: "That guy... he, he had it coming!",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 1104,
                end_ntsc_frame: 1203,
                text: "I didn't do anything.\nHe just came after me!",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 1203,
                end_ntsc_frame: 1296,
                text: "Besides he was making fun of\nme with his eyes!",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 1296,
                end_ntsc_frame: 1386,
                text: "Like that other one...",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 1386,
                end_ntsc_frame: 1500,
                text: "Just for that you killed him?",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 1500,
                end_ntsc_frame: 1614,
                text: "Whadda ya mean \"Just for that\"!",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 1647,
                end_ntsc_frame: 1770,
                text: "Eddie, you can't just kill someone\ncause of the way they looked\nat you...",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 1770,
                end_ntsc_frame: 1896,
                text: "Oh yeah! Why not?",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 1941,
                end_ntsc_frame: 2049,
                text: "'Til now, I always let people walk\nall over me.",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 2049,
                end_ntsc_frame: 2130,
                text: "Just like that stupid dog.",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 2130,
                end_ntsc_frame: 2265,
                text: "He had it coming too!!",
            },
        ],
        "korosu_a" => &[
            FmvSubtitleCue {
                start_ntsc_frame: 249,
                end_ntsc_frame: 375,
                text: "Killin' a person ain't no big deal.",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 375,
                end_ntsc_frame: 480,
                text: "Just put the gun to their head... pow!",
            },
        ],
        "korosu_b" => &[
            FmvSubtitleCue {
                start_ntsc_frame: 24,
                end_ntsc_frame: 78,
                text: "Whadda ya mean \"Just for that\"!",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 78,
                end_ntsc_frame: 237,
                text: "Eddie, you can't just kill someone\ncause of the way they looked\nat you...",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 237,
                end_ntsc_frame: 300,
                text: "Oh yeah! Why not?",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 300,
                end_ntsc_frame: 420,
                text: "'Til now, I always let people walk\nall over me.",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 420,
                end_ntsc_frame: 519,
                text: "Just like that stupid dog.",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 519,
                end_ntsc_frame: 570,
                text: "He had it coming too!!",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 570,
                end_ntsc_frame: 612,
                text: "Eddie!!",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 660,
                end_ntsc_frame: 744,
                text: "He he. I was just jokin', James.",
            },
        ],
        "murder" => &[
            FmvSubtitleCue {
                start_ntsc_frame: 45,
                end_ntsc_frame: 195,
                text: "Are you taping again?\nC'mon...",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 258,
                end_ntsc_frame: 375,
                text: "I don't know why, but I just\nlove it here.",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 375,
                end_ntsc_frame: 459,
                text: "It's so peaceful.",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 459,
                end_ntsc_frame: 501,
                text: "You know what I heard?",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 501,
                end_ntsc_frame: 636,
                text: "This whole area used to be a\nsacred place.",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 636,
                end_ntsc_frame: 723,
                text: "I think I can see why.",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 723,
                end_ntsc_frame: 807,
                text: "It's too bad we have to leave...",
            },
            FmvSubtitleCue {
                start_ntsc_frame: 807,
                end_ntsc_frame: 948,
                text: "Please promise you'll take me\nagain, James.",
            },
        ],
        _ => &[],
    }
}

/// Looks up the active subtitle text for a movie at a given video frame index, automatically
/// rescaling the frame index for 59.94 FPS SH2:EE videos (`Patches/FmvSubtitles.cpp`).
pub fn active_subtitle_for_frame(
    movie_id: &str,
    header: &BinkHeader,
    video_frame: u32,
) -> Option<&'static str> {
    let ntsc_frame = header.subtitle_ntsc_frame(video_frame);
    let cues = fmv_subtitles_for_movie(movie_id);
    for cue in cues {
        if ntsc_frame >= cue.start_ntsc_frame && ntsc_frame <= cue.end_ntsc_frame {
            return Some(cue.text);
        }
    }
    None
}

/// Retail subtitle table of one movie (see [`RETAIL_FMV_SUBTITLES`]).
#[derive(Debug, Clone, Copy)]
pub struct RetailSubtitleTable {
    /// Movie stem.
    pub movie: &'static str,
    /// Stage message file stem (`stage_<stem>_msg_e.mes`) that the owning stage program has loaded.
    pub message_file: &'static str,
    /// First message number (the third argument of `0x4a2fc0`).
    pub first_message: usize,
    /// `{start, end}` NTSC frames per message (the table the stage passes to `0x43dc20`).
    pub frames: &'static [(u16, u16)],
}

/// Subtitle tables extracted from `sh2pc.exe` (callers of `0x4a2fc0` in the stage programs; the exe
/// addresses are noted per entry). Movies not listed (`toilet`, `water`, `gero`, `flash`, endings,
/// credits, ...) have no retail subtitles.
pub const RETAIL_FMV_SUBTITLES: &[RetailSubtitleTable] = &[
    // table at 0x8d1570
    RetailSubtitleTable {
        movie: "rouya",
        message_file: "prison_n",
        first_message: 13,
        frames: &[
            (63, 174),
            (174, 285),
            (348, 495),
            (570, 657),
            (657, 759),
            (795, 891),
            (891, 960),
            (960, 1104),
            (1104, 1203),
            (1203, 1296),
            (1296, 1386),
            (1386, 1500),
            (1500, 1614),
            (1647, 1770),
            (1770, 1896),
            (1941, 2049),
            (2049, 2130),
            (2130, 2265),
        ],
    },
    // table at 0x8d3580
    RetailSubtitleTable {
        movie: "korosu_a",
        message_file: "prison_n",
        first_message: 13,
        frames: &[(249, 375), (375, 480)],
    },
    // table at 0x8d35b8
    RetailSubtitleTable {
        movie: "korosu_b",
        message_file: "prison_n",
        first_message: 25,
        frames: &[
            (24, 78),
            (78, 237),
            (237, 300),
            (300, 420),
            (420, 519),
            (519, 570),
            (570, 612),
            (660, 744),
        ],
    },
    // table at 0x8d54e8
    RetailSubtitleTable {
        movie: "saikai",
        message_file: "hospital_bf_b",
        first_message: 7,
        frames: &[
            (0, 45),
            (45, 90),
            (90, 255),
            (255, 300),
            (348, 477),
            (501, 528),
            (528, 585),
            (585, 678),
            (678, 744),
            (744, 819),
            (819, 918),
            (918, 1035),
            (1035, 1188),
            (1230, 1284),
            (1284, 1344),
            (1344, 1413),
            (1413, 1572),
            (1572, 1692),
            (1692, 1800),
            (1800, 1896),
        ],
    },
    // table at 0x8ddea4
    RetailSubtitleTable {
        movie: "deai",
        message_file: "town_west",
        first_message: 23,
        frames: &[
            (12, 54),
            (246, 327),
            (327, 405),
            (405, 522),
            (522, 582),
            (582, 708),
            (708, 798),
            (798, 873),
            (873, 966),
        ],
    },
    // table at 0x8dddf4
    RetailSubtitleTable {
        movie: "hei",
        message_file: "town_west",
        first_message: 13,
        frames: &[
            (36, 174),
            (174, 210),
            (210, 306),
            (306, 429),
            (429, 522),
            (522, 606),
            (606, 708),
            (708, 774),
            (774, 804),
            (804, 873),
        ],
    },
    // table at 0x8e06c4
    RetailSubtitleTable {
        movie: "knife",
        message_file: "apart_w1f",
        first_message: 12,
        frames: &[(201, 279)],
    },
    // table at 0x8e75f0
    RetailSubtitleTable {
        movie: "hakaba",
        message_file: "forest",
        first_message: 5,
        frames: &[(564, 624), (630, 720), (720, 831), (831, 921), (987, 1026)],
    },
    // table at 0x8c9288
    RetailSubtitleTable {
        movie: "murder",
        message_file: "hotel_3f_f",
        first_message: 5,
        frames: &[
            (45, 195),
            (258, 375),
            (375, 459),
            (459, 501),
            (501, 636),
            (636, 723),
            (723, 807),
            (807, 948),
        ],
    },
];

/// Parses a retail `data/etc/message/*.mes` file into one string per message (pages joined with
/// `'\n'`, lines separated by `'\n'`). Layout (`sh23ds/tools/sh2conv/sh2mes.py`): `u16 count`,
/// `u16 offset[count]` in 16-bit words; text bytes `b < 0xff` are `chr(b + 0x20)`, `ff fd` is a
/// newline, `ff ff` ends a page and other `ff xx` escapes are display codes (dropped).
pub fn parse_mes_messages(d: &[u8]) -> Vec<String> {
    let rd16 = |p: usize| -> Option<usize> {
        d.get(p..p + 2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]) as usize)
    };
    let Some(n) = rd16(0) else {
        return Vec::new();
    };
    let offs: Vec<usize> = (0..n).map_while(|i| rd16(2 + 2 * i)).collect();
    let is_control =
        |p: usize| p + 1 < d.len() && d[p] == 0 && d[p + 1] != 0 && (d[p + 1] & 0x0f) == 0;
    let mut out = Vec::with_capacity(offs.len());
    for (i, &o) in offs.iter().enumerate() {
        let mut p = o * 2;
        let end = offs.get(i + 1).map_or(d.len(), |e| e * 2).min(d.len());
        let mut pages: Vec<String> = Vec::new();
        let mut s = String::new();
        let mut fresh = true;
        while p < end {
            if fresh {
                p += p & 1;
                while is_control(p) {
                    p += 2;
                }
                if p >= end {
                    break;
                }
                fresh = false;
            }
            let b = d[p];
            if b == 0xff && p + 1 < end {
                let c = d[p + 1];
                p += 2;
                match c {
                    0xff => {
                        pages.push(s.trim().to_string());
                        s.clear();
                        fresh = true;
                    }
                    0xfd => s.push('\n'),
                    _ => {}
                }
                continue;
            }
            s.push(if b < 0x60 { (b + 0x20) as char } else { '?' });
            p += 1;
        }
        if !s.is_empty() {
            pages.push(s.trim().to_string());
        }
        pages.retain(|x| !x.is_empty());
        out.push(pages.join("\n"));
    }
    out
}

/// One retail subtitle cue: NTSC frame window plus the message text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetailSubtitleCue {
    /// Exclusive start frame (retail shows the cue once the frame index is greater than this).
    pub start_ntsc_frame: u32,
    /// Inclusive end frame.
    pub end_ntsc_frame: u32,
    /// Subtitle text (lines separated by `'\n'`).
    pub text: String,
}

/// Locates the game root (the folder holding `data/` and `sh2e/`) from a movie path such as
/// `<root>/data/movie/x.bik` or `<root>/sh2e/movie/x.bik`, or searches candidate roots in the workspace.
pub fn game_root_from_movie_path(movie_path: &Path) -> Option<PathBuf> {
    if let Some(parent_root) = movie_path
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
    {
        if retail_messages_available(parent_root) {
            return Some(parent_root.to_path_buf());
        }
    }
    // Check known candidate paths relative to cwd or repository
    let candidates = [
        PathBuf::from("Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2"),
        PathBuf::from("/home/matthewb/Documents/SH2-Port/Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2"),
    ];
    for c in &candidates {
        if retail_messages_available(c) {
            return Some(c.clone());
        }
    }
    None
}

fn read_retail_message_file(root: &Path, stem: &str) -> Option<Vec<u8>> {
    let name = format!("stage_{stem}_msg_e.mes");
    // The Enhanced Edition's copy wins when installed (`sh2e/etc/message`), like the game does.
    ["sh2e/etc/message", "data/etc/message"]
        .iter()
        .find_map(|dir| std::fs::read(root.join(dir).join(&name)).ok())
}

/// True when the retail message folder exists under `root`.
pub fn retail_messages_available(root: &Path) -> bool {
    root.join("data/etc/message").is_dir() || root.join("sh2e/etc/message").is_dir()
}

/// Builds the retail cues of `movie_id` from the `.mes` file in the retail rip at `root`.
pub fn load_retail_subtitles(root: &Path, movie_id: &str) -> Option<Vec<RetailSubtitleCue>> {
    let table = RETAIL_FMV_SUBTITLES.iter().find(|t| t.movie == movie_id)?;
    let msgs = parse_mes_messages(&read_retail_message_file(root, table.message_file)?);
    let mut cues = Vec::with_capacity(table.frames.len());
    for (i, &(s, e)) in table.frames.iter().enumerate() {
        let text = msgs.get(table.first_message + i)?;
        cues.push(RetailSubtitleCue {
            start_ntsc_frame: s as u32,
            end_ntsc_frame: e as u32,
            text: text.clone(),
        });
    }
    Some(cues)
}

type RetailCueCache = Mutex<HashMap<(PathBuf, String), Option<Arc<Vec<RetailSubtitleCue>>>>>;

fn retail_cues_cached(root: &Path, movie_id: &str) -> Option<Arc<Vec<RetailSubtitleCue>>> {
    static CACHE: OnceLock<RetailCueCache> = OnceLock::new();
    let mut map = CACHE.get_or_init(Default::default).lock().ok()?;
    map.entry((root.to_path_buf(), movie_id.to_string()))
        .or_insert_with(|| load_retail_subtitles(root, movie_id).map(Arc::new))
        .clone()
}

/// Subtitle text for a movie frame: the retail table and `.mes` text when the retail rip is found
/// next to `movie_path` (movies without a retail table then show nothing, as in the game),
/// otherwise the built-in [`fmv_subtitles_for_movie`] fallback cues.
pub fn active_subtitle_text(
    movie_id: &str,
    header: &BinkHeader,
    video_frame: u32,
    movie_path: Option<&Path>,
) -> Option<String> {
    let ntsc = header.subtitle_ntsc_frame(video_frame);
    if let Some(root) = movie_path.and_then(game_root_from_movie_path) {
        if retail_messages_available(&root) {
            let cues = retail_cues_cached(&root, movie_id)?;
            return cues
                .iter()
                .find(|c| ntsc > c.start_ntsc_frame && ntsc <= c.end_ntsc_frame)
                .map(|c| c.text.clone());
        }
    }
    active_subtitle_for_frame(movie_id, header, video_frame).map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_synthetic_bink_header_and_subtitle_scaling() {
        let mut raw = vec![0u8; 128];
        raw[0..4].copy_from_slice(b"BIKi");
        raw[4..8].copy_from_slice(&6310272u32.to_le_bytes()); // file_size - 8
        raw[8..12].copy_from_slice(&261u32.to_le_bytes()); // 261 frames
        raw[12..16].copy_from_slice(&40000u32.to_le_bytes());
        raw[16..20].copy_from_slice(&261u32.to_le_bytes());
        raw[20..24].copy_from_slice(&1920u32.to_le_bytes()); // width = 1920
        raw[24..28].copy_from_slice(&1230u32.to_le_bytes()); // height = 1230
        raw[28..32].copy_from_slice(&2997u32.to_le_bytes()); // fps_num = 2997
        raw[32..36].copy_from_slice(&50u32.to_le_bytes()); // fps_den = 50 (59.94 fps)
        raw[36..40].copy_from_slice(&0u32.to_le_bytes());
        raw[40..44].copy_from_slice(&1u32.to_le_bytes()); // 1 audio track
        raw[44..48].copy_from_slice(&145920u32.to_le_bytes()); // max packet
        raw[48..50].copy_from_slice(&48000u16.to_le_bytes()); // 48000 Hz
        raw[50..52].copy_from_slice(&0x7000u16.to_le_bytes()); // stereo + DCT
        raw[52..56].copy_from_slice(&0u32.to_le_bytes()); // track ID 0

        let hdr = parse_bink_header(&raw).expect("Should parse valid BIKi header");
        assert_eq!(hdr.magic, *b"BIKi");
        assert_eq!(hdr.width, 1920);
        assert_eq!(hdr.height, 1230);
        assert_eq!(hdr.frame_count, 261);
        assert!((hdr.fps() - 59.94).abs() < 0.01);
        assert!(hdr.is_hd_enhanced());
        assert_eq!(hdr.active_16_9_crop(), (75, 1080));
        assert_eq!(hdr.audio_tracks.len(), 1);
        assert_eq!(hdr.audio_tracks[0].sample_rate, 48000);
        assert_eq!(hdr.audio_tracks[0].channels, 2);
        assert!(hdr.audio_tracks[0].is_dct);

        // At 59.94 FPS, frame 200 corresponds to frame 100 in NTSC 29.97 FPS subtitle timing
        assert_eq!(hdr.subtitle_ntsc_frame(200), 100);
        assert_eq!(
            active_subtitle_for_frame("murder", &hdr, 200),
            Some("Are you taping again?\nC'mon...")
        );
    }

    #[test]
    fn test_synthetic_yuv420_frame_bt601_color_bars() {
        let frame = Yuv420Frame::generate_synthetic_test_frame(640, 480, 0);
        assert_eq!(frame.y_plane.len(), 640 * 480);
        assert_eq!(frame.u_plane.len(), 320 * 240);
        assert_eq!(frame.v_plane.len(), 320 * 240);

        // Bar 0 (leftmost, x=20, y=50) is White (255, 255, 255)
        let white_px = frame.sample_rgba_bt601(20, 50);
        assert!(white_px[0] > 245 && white_px[1] > 245 && white_px[2] > 245);

        // Bar 7 (rightmost, x=620, y=50) is Black (0, 0, 0)
        let black_px = frame.sample_rgba_bt601(620, 50);
        assert!(black_px[0] < 10 && black_px[1] < 10 && black_px[2] < 10);
    }

    #[test]
    fn test_parse_mes_synthetic() {
        // 1 message at word offset 2: control word, "Hi" (0x28 0x49 -> 'H','i'), ff fd, "A", ff ff
        let mut d = vec![1, 0, 2, 0, 0x00, 0x80];
        d.extend_from_slice(&[0x28, 0x49, 0xff, 0xfd, 0x21, 0xff, 0xff, 0x00]);
        assert_eq!(parse_mes_messages(&d), vec!["Hi\nA".to_string()]);
    }

    #[test]
    fn test_retail_tables_are_sorted_and_nonempty() {
        for t in RETAIL_FMV_SUBTITLES {
            assert!(!t.frames.is_empty());
            assert!(t.frames.iter().all(|&(s, e)| s < e));
            assert!(t.frames.windows(2).all(|w| w[0].1 <= w[1].0));
        }
    }

    #[test]
    fn test_retail_subtitles_when_rip_present() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../Silent-Hill-2-Restless-Dreams_Win_EN_Full-Rip/Silent Hill 2");
        if !retail_messages_available(&root) {
            return; // retail assets absent (CI)
        }
        let cues = load_retail_subtitles(&root, "deai").expect("deai cues");
        assert_eq!(cues.len(), 9);
        assert_eq!(cues[0].text, "Mary?");
        assert_eq!(cues[7].text, "My name... is Maria.");
        assert!(load_retail_subtitles(&root, "toilet").is_none());
    }
}
