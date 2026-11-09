//! Per-puzzle sprite quad emitters for [`super::CutawayOverlayEngine`] (`prog_overlay.c`).

use crate::events::{EventFlags, PuzzleState, PZ_CLOCK, PZ_WHEEL, PZ_WHEEL_GOAL};

use super::tables::{
    original_sheet_dimensions, read_i16, read_i32, OV_BOX, OV_CLOCK_UV, OV_COIN, OV_DRUM_ABC,
    OV_DRUM_POS, OV_MARKER, OV_TEIEN, OV_VIDEO, OV_WALL,
};
use super::{CutawayOverlayEngine, OverlaySpriteQuad};

impl CutawayOverlayEngine {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn emit_rect(
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
            vertices: [
                (x0, y0, u0, v0),
                (x1, y0, u1, v0),
                (x1, y1, u1, v1),
                (x0, y1, u0, v1),
            ],
            alpha,
        });
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn emit_quad(
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
    pub(super) fn draw_clock(&mut self, puzzle: &PuzzleState, open: bool) {
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
    pub(super) fn draw_safe(&mut self, flags: &EventFlags) {
        let mut pos = 0i32;
        for b in 0..5usize {
            if flags.get(103 + b) {
                pos |= 1 << b;
            }
        }
        let a = (-pos as f32) * (std::f32::consts::TAU / 20.0);
        let cs = a.cos();
        let sn = a.sin();
        let c = [
            (-64.0f32, -64.0f32),
            (64.0, -64.0),
            (64.0, 64.0),
            (-64.0, 64.0),
        ];
        let mut pts = [(0.0f32, 0.0f32); 4];
        for k in 0..4 {
            pts[k].0 = (c[k].0 * cs - c[k].1 * sn) * 0.8 - 1.0;
            pts[k].1 = c[k].0 * sn + c[k].1 * cs - 2.0;
        }
        self.emit_quad("apt/p_safe_close2", pts, 0.0, 0.0, 128.0, 128.0, 1.0);
    }

    // ---------------------------------------------------------------- coins (0x597ce0)
    pub(super) fn draw_coins(&mut self, flags: &EventFlags) {
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
    pub(super) fn draw_box(&mut self, flags: &EventFlags, puzzle: &PuzzleState) {
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

    pub(super) fn draw_box_wall(&mut self, puzzle: &PuzzleState) {
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
    pub(super) fn draw_quiz(&mut self, flags: &EventFlags) {
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
    pub(super) fn draw_drums(&mut self, flags: &EventFlags, puzzle: &PuzzleState) {
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
                self.emit_rect(
                    "htl/dram",
                    x,
                    y,
                    x + 28.0,
                    y + h,
                    u,
                    v,
                    u + 32.0,
                    v + h,
                    1.0,
                );
            }
        }
    }

    // ---------------------------------------------------------------- hotel egg doors (0x57bd40)
    pub(super) fn draw_egg(&mut self, flags: &EventFlags) {
        if flags.get(380) {
            self.emit_rect(
                "htl/egg", -157.0, -41.0, -109.0, 14.0, 0.0, 0.0, 48.0, 55.0, 1.0,
            );
        }
        if flags.get(381) {
            self.emit_rect(
                "htl/egg", 94.0, -40.0, 142.0, 15.0, 0.0, 0.0, 48.0, 55.0, 1.0,
            );
        }
    }

    // ---------------------------------------------------------------- Baldwin tombstone (0x571f50)
    pub(super) fn draw_tombstone(&mut self, flags: &EventFlags) {
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
                let mut rot = if flags.get(694 + b * 2) {
                    1usize
                } else {
                    0usize
                } | if flags.get(695 + b * 2) {
                    2usize
                } else {
                    0usize
                };
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
    pub(super) fn draw_video(&mut self, flags: &EventFlags) {
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

    pub(super) fn draw_marker(&mut self, flags: &EventFlags, riddle_difficulty: u8) {
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
            self.emit_rect(
                "htl/maker",
                x,
                y,
                x + 24.0,
                y + 24.0,
                u,
                v,
                u + 24.0,
                v + 24.0,
                1.0,
            );
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
    pub(super) fn draw_elevator(&mut self, tab: &[u8; 96], floor_mask: i32) {
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
