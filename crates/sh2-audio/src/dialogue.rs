//! Dialogue playback state and `.mes` subtitle page synchronization (`SH2-55`).

use sh2_assets::DialogueMessage;
use std::time::Duration;

/// Active dialogue playback state tracking subtitle synchronization and voice timing.
#[derive(Debug, Clone)]
pub struct ActiveDialogue {
    /// Current dialogue message being spoken.
    pub message: DialogueMessage,
    /// Voice track identifier or index if known.
    pub voice_id: Option<u32>,
    /// Total duration of the voice line.
    pub total_duration: Duration,
    /// Elapsed playback time in seconds.
    pub elapsed: Duration,
    /// Index of the current active page of subtitle text.
    pub current_page_index: usize,
    /// Whether this dialogue has completed playback.
    pub is_finished: bool,
}

impl ActiveDialogue {
    /// Returns the currently displayed subtitle page text, if any.
    pub fn current_subtitle(&self) -> Option<&str> {
        self.message
            .pages
            .get(self.current_page_index)
            .map(|s| s.as_str())
    }

    /// Advances the dialogue timer and calculates active subtitle page based on progress.
    pub fn update(&mut self, dt: Duration) {
        if self.is_finished {
            return;
        }

        self.elapsed += dt;
        if self.elapsed >= self.total_duration {
            self.is_finished = true;
            return;
        }

        if !self.message.pages.is_empty() {
            let num_pages = self.message.pages.len();
            let page_duration = self.total_duration.as_secs_f32() / (num_pages as f32);
            let cur_idx = (self.elapsed.as_secs_f32() / page_duration).floor() as usize;
            self.current_page_index = cur_idx.min(num_pages - 1);
        }
    }
}
