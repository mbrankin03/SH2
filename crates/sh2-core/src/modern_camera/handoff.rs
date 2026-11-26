//! Camera mode enums, `R3` cycle presets, cross-mode smoothstep transitions, and scripted
//! `.dds` cutscene (`SH2-63`) / 2D puzzle cutaway (`SH2-15`) handoff state machines (`SH2-93`).

use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};

use crate::camera::CameraSnapshot;
use crate::cutaway::{CutawayOverlayEngine, OverlayKind};
use crate::cutscene::{CutsceneCameraSnapshot, DramaDemoPlayer, DramaDemoStep};

/// Active gameplay camera framing paradigm (`SH2-93`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum GameplayCameraMode {
    /// Authentic 1:1 PlayStation 2 `.cam` rail, chase, locus-circle, and fixed framing.
    #[default]
    ClassicPs2Rail,
    /// Modern damped Over-the-Shoulder 3D camera with `.cld` spring-arm wall/ceiling collision.
    OverTheShoulder,
    /// Head-bone anchored First-Person view with clamped pitch and upper-body clip prevention.
    FirstPerson,
}

impl GameplayCameraMode {
    /// User-facing label for the Options menu and HUD.
    pub const fn label(self) -> &'static str {
        match self {
            Self::ClassicPs2Rail => "Classic PS2 Rail",
            Self::OverTheShoulder => "Over-the-Shoulder 3D",
            Self::FirstPerson => "First-Person (FPS)",
        }
    }

    /// Returns `true` when using a modern free-look camera (`OverTheShoulder` or `FirstPerson`).
    #[inline]
    pub const fn is_modern(self) -> bool {
        !matches!(self, Self::ClassicPs2Rail)
    }

    /// Cycles forward across all three camera modes (`ClassicPs2Rail -> OverTheShoulder -> FirstPerson`).
    pub const fn next(self) -> Self {
        match self {
            Self::ClassicPs2Rail => Self::OverTheShoulder,
            Self::OverTheShoulder => Self::FirstPerson,
            Self::FirstPerson => Self::ClassicPs2Rail,
        }
    }

    /// Cycles backward across all three camera modes.
    pub const fn prev(self) -> Self {
        match self {
            Self::ClassicPs2Rail => Self::FirstPerson,
            Self::OverTheShoulder => Self::ClassicPs2Rail,
            Self::FirstPerson => Self::OverTheShoulder,
        }
    }

    /// Cycles to the next camera mode within the user's configured `R3` [`CameraCyclePreset`].
    pub const fn cycle_with_preset(self, preset: CameraCyclePreset) -> Self {
        match preset {
            CameraCyclePreset::ClassicOnly => Self::ClassicPs2Rail,
            CameraCyclePreset::AllThree => self.next(),
            CameraCyclePreset::ClassicAndOts => match self {
                Self::ClassicPs2Rail => Self::OverTheShoulder,
                Self::OverTheShoulder | Self::FirstPerson => Self::ClassicPs2Rail,
            },
            CameraCyclePreset::ClassicAndFirstPerson => match self {
                Self::ClassicPs2Rail => Self::FirstPerson,
                Self::OverTheShoulder | Self::FirstPerson => Self::ClassicPs2Rail,
            },
            CameraCyclePreset::OtsAndFirstPerson => match self {
                Self::OverTheShoulder => Self::FirstPerson,
                Self::ClassicPs2Rail | Self::FirstPerson => Self::OverTheShoulder,
            },
        }
    }
}

/// Configurable subset of camera modes cycled when pressing `R3` in-game (`SH2-93`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CameraCyclePreset {
    /// Strict 1:1 Vanilla PS2 lock: `R3` never switches out of `ClassicPs2Rail`.
    ClassicOnly,
    /// Cycle all three modes: `ClassicPs2Rail -> OverTheShoulder -> FirstPerson`.
    #[default]
    AllThree,
    /// Toggle strictly between `ClassicPs2Rail` and `OverTheShoulder`.
    ClassicAndOts,
    /// Toggle strictly between `ClassicPs2Rail` and `FirstPerson`.
    ClassicAndFirstPerson,
    /// Toggle strictly between `OverTheShoulder` and `FirstPerson`.
    OtsAndFirstPerson,
}

impl CameraCyclePreset {
    /// User-facing label for the Options menu.
    pub const fn label(self) -> &'static str {
        match self {
            Self::ClassicOnly => "Classic PS2 Only (Locked)",
            Self::AllThree => "Rail / OTS / FPS",
            Self::ClassicAndOts => "Rail / OTS Only",
            Self::ClassicAndFirstPerson => "Rail / FPS Only",
            Self::OtsAndFirstPerson => "OTS / FPS Only",
        }
    }

    /// Next cyclical `R3` preset.
    pub const fn next(self) -> Self {
        match self {
            Self::AllThree => Self::ClassicAndOts,
            Self::ClassicAndOts => Self::ClassicAndFirstPerson,
            Self::ClassicAndFirstPerson => Self::OtsAndFirstPerson,
            Self::OtsAndFirstPerson => Self::ClassicOnly,
            Self::ClassicOnly => Self::AllThree,
        }
    }

    /// Previous cyclical `R3` preset.
    pub const fn prev(self) -> Self {
        match self {
            Self::AllThree => Self::ClassicOnly,
            Self::ClassicAndOts => Self::AllThree,
            Self::ClassicAndFirstPerson => Self::ClassicAndOts,
            Self::OtsAndFirstPerson => Self::ClassicAndFirstPerson,
            Self::ClassicOnly => Self::OtsAndFirstPerson,
        }
    }
}

/// Cross-mode smoothstep blend state triggered by `R3` live camera swaps (`SH2-93`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ModeTransitionState {
    /// Fully settled in `active_mode`.
    #[default]
    Settled,
    /// Smoothly Hermite-blending from `from_snapshot` into the newly selected `active_mode`.
    Crossfading {
        /// Outgoing camera snapshot at the instant `R3` was pressed.
        from_snapshot: CameraSnapshot,
        /// Outgoing near-clip plane distance in SH2 world units.
        from_near_clip: f32,
        /// Ticks elapsed since mode swap (`0..duration_ticks` at 60 Hz).
        elapsed_ticks: u16,
        /// Total transition duration in 60 Hz ticks.
        duration_ticks: u16,
    },
}

/// Typed camera handoff state machine for `.dds` cutscenes (`SH2-63`) & 2D puzzle cutaways (`SH2-15`).
#[derive(Debug, Clone, Copy, Default)]
pub enum ExternalCameraHandoff {
    /// Standard gameplay camera (`ClassicPs2Rail`, `OverTheShoulder`, or `FirstPerson`).
    #[default]
    GameplayActive,
    /// Smoothly blending from gameplay camera into an authored `.dds` cutscene camera.
    BlendingIntoCutscene {
        /// Outgoing gameplay snapshot.
        from_snapshot: CameraSnapshot,
        /// Active `.dds` cutscene target snapshot.
        target: CutsceneCameraSnapshot,
        /// Elapsed 60 Hz blend ticks.
        elapsed_ticks: u16,
        /// Total 60 Hz blend duration ticks.
        duration_ticks: u16,
    },
    /// Locked directly onto the active `.dds` `DramaDemoPlayer` camera (`SH2-63`).
    CutsceneLocked {
        /// Current `.dds` cutscene snapshot.
        current: CutsceneCameraSnapshot,
    },
    /// Locked during a 2D interactive puzzle/memo cutaway (`CutawayOverlayEngine` `SH2-15`).
    CutawayPuzzleLocked {
        /// Gameplay snapshot frozen when the cutaway opened.
        held_snapshot: CameraSnapshot,
        /// Active cutaway overlay kind.
        overlay_kind: OverlayKind,
    },
    /// Smoothly blending out of a completed cutscene or closed cutaway back into `active_mode`.
    BlendingOutOfExternal {
        /// Final external snapshot at handoff exit.
        from_snapshot: CameraSnapshot,
        /// Elapsed 60 Hz blend ticks.
        elapsed_ticks: u16,
        /// Total 60 Hz blend duration ticks.
        duration_ticks: u16,
    },
}

impl ModeTransitionState {
    /// Steps the cross-mode Hermite blend for one 60 Hz tick, updating `snapshot` and `near_clip`.
    pub fn step(&mut self, snapshot: &mut CameraSnapshot, near_clip: &mut f32) {
        if let Self::Crossfading {
            from_snapshot,
            from_near_clip,
            elapsed_ticks,
            duration_ticks,
        } = *self
        {
            let next_elapsed = elapsed_ticks.saturating_add(1);
            let t = (next_elapsed as f32) / (duration_ticks.max(1) as f32);
            *snapshot = blend_snapshots(&from_snapshot, snapshot, t);
            let s = smoothstep01(t);
            *near_clip = from_near_clip + (*near_clip - from_near_clip) * s;
            *self = if next_elapsed >= duration_ticks {
                Self::Settled
            } else {
                Self::Crossfading {
                    from_snapshot,
                    from_near_clip,
                    elapsed_ticks: next_elapsed,
                    duration_ticks,
                }
            };
        }
    }
}

impl ExternalCameraHandoff {
    /// Returns `true` when free-look input and `R3` mode cycling should be suppressed.
    #[inline]
    pub const fn locks_camera_controls(&self) -> bool {
        matches!(
            self,
            Self::BlendingIntoCutscene { .. }
                | Self::CutsceneLocked { .. }
                | Self::CutawayPuzzleLocked { .. }
        )
    }

    /// Evaluates and advances the external cutscene/cutaway handoff for one 60 Hz tick.
    ///
    /// Returns `(final_snapshot, overrides_reticle_and_near_clip, is_hard_cut)`.
    pub fn step(&mut self, base_snap: CameraSnapshot) -> (CameraSnapshot, bool, bool) {
        match *self {
            Self::GameplayActive => (base_snap, false, false),
            Self::BlendingIntoCutscene {
                from_snapshot,
                target,
                elapsed_ticks,
                duration_ticks,
            } => {
                let next_elapsed = elapsed_ticks.saturating_add(1);
                let cs_snap = snapshot_from_cutscene(&target);
                let out = blend_snapshots(
                    &from_snapshot,
                    &cs_snap,
                    (next_elapsed as f32) / (duration_ticks.max(1) as f32),
                );
                *self = if next_elapsed >= duration_ticks {
                    Self::CutsceneLocked { current: target }
                } else {
                    Self::BlendingIntoCutscene {
                        from_snapshot,
                        target,
                        elapsed_ticks: next_elapsed,
                        duration_ticks,
                    }
                };
                (out, true, false)
            }
            Self::CutsceneLocked { current } => {
                (snapshot_from_cutscene(&current), true, current.is_hard_cut)
            }
            Self::CutawayPuzzleLocked { held_snapshot, .. } => (held_snapshot, true, false),
            Self::BlendingOutOfExternal {
                from_snapshot,
                elapsed_ticks,
                duration_ticks,
            } => {
                let next_elapsed = elapsed_ticks.saturating_add(1);
                let out = blend_snapshots(
                    &from_snapshot,
                    &base_snap,
                    (next_elapsed as f32) / (duration_ticks.max(1) as f32),
                );
                *self = if next_elapsed >= duration_ticks {
                    Self::GameplayActive
                } else {
                    Self::BlendingOutOfExternal {
                        from_snapshot,
                        elapsed_ticks: next_elapsed,
                        duration_ticks,
                    }
                };
                (out, false, false)
            }
        }
    }

    /// Synchronizes handoff state with an optional `.dds` [`DramaDemoPlayer`] (`SH2-63`).
    pub fn sync_cutscene(
        &mut self,
        demo_player: Option<&DramaDemoPlayer>,
        curr_snapshot: CameraSnapshot,
        blend_ticks: u16,
    ) {
        match demo_player {
            Some(player) if player.step != DramaDemoStep::Finished => {
                let cs = player.camera;
                match *self {
                    Self::GameplayActive
                    | Self::BlendingOutOfExternal { .. }
                    | Self::CutawayPuzzleLocked { .. } => {
                        if cs.is_hard_cut || blend_ticks == 0 {
                            *self = Self::CutsceneLocked { current: cs };
                        } else {
                            *self = Self::BlendingIntoCutscene {
                                from_snapshot: curr_snapshot,
                                target: cs,
                                elapsed_ticks: 0,
                                duration_ticks: blend_ticks,
                            };
                        }
                    }
                    Self::BlendingIntoCutscene {
                        from_snapshot,
                        elapsed_ticks,
                        duration_ticks,
                        ..
                    } => {
                        if cs.is_hard_cut {
                            *self = Self::CutsceneLocked { current: cs };
                        } else {
                            *self = Self::BlendingIntoCutscene {
                                from_snapshot,
                                target: cs,
                                elapsed_ticks,
                                duration_ticks,
                            };
                        }
                    }
                    Self::CutsceneLocked { .. } => {
                        *self = Self::CutsceneLocked { current: cs };
                    }
                }
            }
            _ => {
                if let Self::BlendingIntoCutscene { target, .. }
                | Self::CutsceneLocked { current: target } = *self
                {
                    *self = Self::BlendingOutOfExternal {
                        from_snapshot: snapshot_from_cutscene(&target),
                        elapsed_ticks: 0,
                        duration_ticks: blend_ticks.max(1),
                    };
                }
            }
        }
    }

    /// Synchronizes handoff state with [`CutawayOverlayEngine`] (`SH2-15`).
    pub fn sync_cutaway(
        &mut self,
        cutaway: &CutawayOverlayEngine,
        curr_snapshot: CameraSnapshot,
        blend_ticks: u16,
    ) {
        let kind = cutaway.kind();
        if kind != OverlayKind::None {
            if !matches!(
                *self,
                Self::BlendingIntoCutscene { .. } | Self::CutsceneLocked { .. }
            ) {
                let held = match *self {
                    Self::CutawayPuzzleLocked { held_snapshot, .. } => held_snapshot,
                    _ => curr_snapshot,
                };
                *self = Self::CutawayPuzzleLocked {
                    held_snapshot: held,
                    overlay_kind: kind,
                };
            }
        } else if let Self::CutawayPuzzleLocked { held_snapshot, .. } = *self {
            *self = Self::BlendingOutOfExternal {
                from_snapshot: held_snapshot,
                elapsed_ticks: 0,
                duration_ticks: blend_ticks.max(1),
            };
        }
    }
}

/// Hermite cubic smoothstep $3t^2 - 2t^3$ clamped to $[0, 1]$.
#[inline]
pub fn smoothstep01(t: f32) -> f32 {
    let x = t.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Smoothly interpolates between two [`CameraSnapshot`]s using Hermite smoothstep easing.
#[inline]
pub fn blend_snapshots(a: &CameraSnapshot, b: &CameraSnapshot, alpha: f32) -> CameraSnapshot {
    let s = smoothstep01(alpha);
    CameraSnapshot {
        eye: a.eye.lerp(b.eye, s),
        target: a.target.lerp(b.target, s),
        up: a.up.lerp(b.up, s).normalize_or(Vec3::new(0.0, -1.0, 0.0)),
        fov_y: a.fov_y + (b.fov_y - a.fov_y) * s,
    }
}

/// Converts an authored `.dds` [`CutsceneCameraSnapshot`] (including view-axis roll) into a [`CameraSnapshot`].
#[inline]
pub fn snapshot_from_cutscene(cs: &CutsceneCameraSnapshot) -> CameraSnapshot {
    let fwd = (cs.interest - cs.eye).normalize_or(Vec3::Z);
    let base_up = Vec3::new(0.0, -1.0, 0.0);
    let rolled_up = Mat4::from_axis_angle(fwd, cs.roll_radians)
        .transform_vector3(base_up)
        .normalize_or(base_up);
    CameraSnapshot {
        eye: cs.eye,
        target: cs.interest,
        up: rolled_up,
        fov_y: cs.fov_y_radians,
    }
}
