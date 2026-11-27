//! Bounded LRU cache for pre-resampled Q12 glyph coverage masks (`SH2-177`).

use std::collections::HashMap;
use std::sync::Arc;

/// Bounded capacity for cached resampled glyph coverage masks (`SH2-177`).
pub(super) const GLYPH_CACHE_CAPACITY: usize = 512;

/// Cache key identifying a resampled glyph mask by `(glyph_id, px_height, x_scale.to_bits())`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct GlyphCacheKey {
    pub(super) glyph_id: usize,
    pub(super) px_height: usize,
    pub(super) x_scale_bits: u32,
}

impl GlyphCacheKey {
    #[inline]
    pub(super) fn new(glyph_id: usize, px_height: usize, clamped_x_scale: f32) -> Self {
        Self {
            glyph_id,
            px_height,
            x_scale_bits: clamped_x_scale.to_bits(),
        }
    }
}

/// Pre-resampled Q12 coverage mask (`0..=4096`) for one glyph at a specific `(px_height, x_scale)`.
pub(super) struct CachedGlyphMask {
    pub(super) width: usize,
    pub(super) height: usize,
    pub(super) coverage: Vec<u16>,
}

/// Bounded LRU cache of resampled glyph coverage masks.
#[derive(Default)]
pub(super) struct GlyphMaskCache {
    entries: HashMap<GlyphCacheKey, (Arc<CachedGlyphMask>, u64)>,
    tick: u64,
}

impl GlyphMaskCache {
    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(super) fn clear(&mut self) {
        self.entries.clear();
        self.tick = 0;
    }

    pub(super) fn get(&mut self, key: GlyphCacheKey) -> Option<Arc<CachedGlyphMask>> {
        self.tick = self.tick.wrapping_add(1);
        let tick = self.tick;
        if let Some((mask, last_used)) = self.entries.get_mut(&key) {
            *last_used = tick;
            return Some(Arc::clone(mask));
        }
        None
    }

    pub(super) fn insert(&mut self, key: GlyphCacheKey, mask: Arc<CachedGlyphMask>) {
        self.tick = self.tick.wrapping_add(1);
        let tick = self.tick;
        if self.entries.len() >= GLYPH_CACHE_CAPACITY && !self.entries.contains_key(&key) {
            if let Some(evict_key) = self
                .entries
                .iter()
                .min_by_key(|(_, (_, last_used))| *last_used)
                .map(|(k, _)| *k)
            {
                self.entries.remove(&evict_key);
            }
        }
        self.entries.insert(key, (mask, tick));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atlas_font::{AtlasFont, Sh2TextColor};
    use crate::font::TextRasterizer;

    fn dummy_mask(val: u16) -> Arc<CachedGlyphMask> {
        Arc::new(CachedGlyphMask {
            width: 1,
            height: 1,
            coverage: vec![val],
        })
    }

    #[test]
    fn lru_cache_hits_recency_eviction_and_clear() {
        let mut cache = GlyphMaskCache::default();
        assert_eq!(cache.len(), 0);

        // Fill to capacity
        for i in 0..GLYPH_CACHE_CAPACITY {
            let key = GlyphCacheKey::new(i, 32, 1.0);
            cache.insert(key, dummy_mask(i as u16));
        }
        assert_eq!(cache.len(), GLYPH_CACHE_CAPACITY);

        // Touch key 0 so key 1 becomes the least-recently-used entry
        let k0 = GlyphCacheKey::new(0, 32, 1.0);
        let k1 = GlyphCacheKey::new(1, 32, 1.0);
        let hit0 = cache.get(k0).expect("key 0 must hit");
        assert_eq!(hit0.coverage[0], 0);

        // Insert a brand-new key; key 1 should be evicted while key 0 survives
        let k_new = GlyphCacheKey::new(GLYPH_CACHE_CAPACITY, 32, 1.0);
        cache.insert(k_new, dummy_mask(999));
        assert_eq!(cache.len(), GLYPH_CACHE_CAPACITY);
        assert!(cache.get(k1).is_none(), "LRU key 1 should be evicted");
        assert!(cache.get(k0).is_some(), "recently used key 0 must remain");
        assert_eq!(cache.get(k_new).unwrap().coverage[0], 999);

        cache.clear();
        assert_eq!(cache.len(), 0);
        assert!(cache.get(k0).is_none());
    }

    #[test]
    fn atlas_font_glyph_cache_hits_and_bounded_eviction() {
        let font = AtlasFont::load_embedded();
        font.clear_glyph_cache();
        assert_eq!(font.glyph_cache_len(), 0);

        let text = "SILENT HILL 2: Restless Dreams";
        let mut cached = TextRasterizer::new(512, 80);
        cached.fill_rect(0, 0, 512, 80, [36, 42, 48, 220]);

        font.draw_line_styled_aspect(
            &mut cached,
            text,
            (12, 16),
            34,
            1.25,
            Sh2TextColor::GoldSelected,
            true,
        );

        // Cache count equals number of unique non-space characters in `text` (reused across shadow + fg)
        let unique_chars: std::collections::HashSet<char> =
            text.chars().filter(|&c| c != ' ').collect();
        assert_eq!(font.glyph_cache_len(), unique_chars.len());

        // Redrawing same text hits cache with zero new entries and identical pixels
        let mut cached2 = TextRasterizer::new(512, 80);
        cached2.fill_rect(0, 0, 512, 80, [36, 42, 48, 220]);
        font.draw_line_styled_aspect(
            &mut cached2,
            text,
            (12, 16),
            34,
            1.25,
            Sh2TextColor::GoldSelected,
            true,
        );
        assert_eq!(font.glyph_cache_len(), unique_chars.len());
        assert_eq!(cached.pixels, cached2.pixels);

        // Eviction test: insert more than GLYPH_CACHE_CAPACITY (512) unique `(glyph, height, scale)` keys
        let mut scratch = TextRasterizer::new(64, 64);
        for h in 10..=40 {
            font.draw_line(
                &mut scratch,
                "ABCDEFGHIJKLMNOPQRST",
                (0, 0),
                h,
                [255; 4],
                false,
            );
        }
        assert!(
            font.glyph_cache_len() <= GLYPH_CACHE_CAPACITY,
            "cache exceeded capacity: {} > {}",
            font.glyph_cache_len(),
            GLYPH_CACHE_CAPACITY
        );
        assert_eq!(font.glyph_cache_len(), GLYPH_CACHE_CAPACITY);
    }
}
