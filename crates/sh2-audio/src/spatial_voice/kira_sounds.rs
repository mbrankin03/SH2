//! Kira [`SoundData`] & [`Sound`] adapters (`SpatialDrySoundData` & `SpatialWetSendSoundData`) (`SH2-84`).

use std::convert::Infallible;
use std::sync::Arc;

use kira::{
    clock::clock_info::ClockInfoProvider,
    modulator::value_provider::ModulatorValueProvider,
    sound::{Sound, SoundData},
    track::TrackId,
    Frame, OutputDestination,
};

use super::{
    SpatialDryWetCore, SpatialVoiceControl, SpatialVoiceHandle, SpatialVoiceInitParams,
    SpatialWetSendCore,
};
use crate::looping::LoopRegionSamples;

/// Kira [`SoundData`] descriptor for the dry occluded + HRTF/PS2-panned 3D SFX stream (`SH2-84`).
pub struct SpatialDrySoundData {
    /// Pre-decoded 48 kHz stereo PCM frames (`Arc`-shared with the parallel wet send).
    pub frames: Arc<[Frame]>,
    /// Source buffer sample rate in `Hz` (typically `48_000`).
    pub sample_rate: u32,
    /// Optional sample-accurate loop region in source frames.
    pub loop_region: Option<LoopRegionSamples>,
    /// Initial spatial, distance, and `.cld` occlusion parameters.
    pub init: SpatialVoiceInitParams,
    /// Lock-free atomic control block shared with gameplay and the wet send stream.
    pub control: Arc<SpatialVoiceControl>,
    /// Destination Kira dry SFX sub-track ID.
    pub destination: TrackId,
}

impl SoundData for SpatialDrySoundData {
    type Error = Infallible;
    type Handle = SpatialVoiceHandle;

    fn into_sound(self) -> Result<(Box<dyn Sound>, Self::Handle), Self::Error> {
        let occ_enabled = self.control.is_occlusion_enabled();
        let handle = SpatialVoiceHandle::new(Arc::clone(&self.control));
        let sound = SpatialDrySound {
            core: SpatialDryWetCore::new_with_occlusion_enabled(
                self.frames,
                self.sample_rate,
                self.loop_region,
                &self.init,
                occ_enabled,
            ),
            control: self.control,
            destination: OutputDestination::Track(self.destination),
            approx_fs: self.sample_rate.max(8_000) as f32,
        };
        Ok((Box::new(sound), handle))
    }
}

struct SpatialDrySound {
    core: SpatialDryWetCore,
    control: Arc<SpatialVoiceControl>,
    destination: OutputDestination,
    approx_fs: f32,
}

impl Sound for SpatialDrySound {
    fn output_destination(&mut self) -> OutputDestination {
        self.destination
    }

    fn on_start_processing(&mut self) {
        self.core.sync_from_control(&self.control, self.approx_fs);
    }

    fn process(
        &mut self,
        dt: f64,
        _clock_info_provider: &ClockInfoProvider,
        _modulator_value_provider: &ModulatorValueProvider,
    ) -> Frame {
        if dt.is_finite() && dt > 1e-7 {
            self.approx_fs = ((1.0 / dt) as f32).clamp(8_000.0, 384_000.0);
        }
        let (dry, _wet) = self.core.process_pair(dt, &self.control);
        dry
    }

    fn finished(&self) -> bool {
        self.core.is_finished()
    }
}

/// Kira [`SoundData`] descriptor for the parallel wet corridor reverb send stream (`SH2-84`).
pub struct SpatialWetSendSoundData {
    /// Pre-decoded 48 kHz stereo PCM frames (`Arc`-shared with the dry stream).
    pub frames: Arc<[Frame]>,
    /// Source buffer sample rate in `Hz` (typically `48_000`).
    pub sample_rate: u32,
    /// Optional sample-accurate loop region in source frames.
    pub loop_region: Option<LoopRegionSamples>,
    /// Initial spatial, distance, and wet bleed parameters.
    pub init: SpatialVoiceInitParams,
    /// Lock-free atomic control block shared with gameplay and the dry stream.
    pub control: Arc<SpatialVoiceControl>,
    /// Destination Kira 100%-wet corridor reverb sub-track ID.
    pub reverb_bus: TrackId,
}

impl SoundData for SpatialWetSendSoundData {
    type Error = Infallible;
    type Handle = ();

    fn into_sound(self) -> Result<(Box<dyn Sound>, Self::Handle), Self::Error> {
        let sound = SpatialWetSendSound {
            core: SpatialWetSendCore::new(
                self.frames,
                self.sample_rate,
                self.loop_region,
                &self.init,
            ),
            control: self.control,
            destination: OutputDestination::Track(self.reverb_bus),
        };
        Ok((Box::new(sound), ()))
    }
}

struct SpatialWetSendSound {
    core: SpatialWetSendCore,
    control: Arc<SpatialVoiceControl>,
    destination: OutputDestination,
}

impl Sound for SpatialWetSendSound {
    fn output_destination(&mut self) -> OutputDestination {
        self.destination
    }

    fn process(
        &mut self,
        dt: f64,
        _clock_info_provider: &ClockInfoProvider,
        _modulator_value_provider: &ModulatorValueProvider,
    ) -> Frame {
        self.core.process_wet_only(dt, &self.control)
    }

    fn finished(&self) -> bool {
        self.core.is_finished()
    }
}
