//! Bounded LRU cache for decoded cutaway textures (`SH2-175`).
//!
//! Prevents unbounded RAM accumulation across long Campaign / Story Progression sessions when
//! viewing many high-resolution (`2048x2048` / `4096x4096`) cutaway stills.

use sh2_assets::CutawayTextureAsset;
use sh2_core::CutawayQuality;

/// Default maximum number of decoded cutaway textures retained in RAM simultaneously.
pub(crate) const DEFAULT_CUTAWAY_CACHE_CAPACITY: usize = 8;

/// Bounded least-recently-used cache keyed by `(pic_or_sheet_name, CutawayQuality)`.
///
/// Entries are ordered from least-recently-used (front) to most-recently-used (back). Because
/// capacity is small (`<= 8` entries), a contiguous `Vec` avoids hashing and temporary `String`
/// allocations on lookup while keeping eviction O(1)-ish with zero heap churn.
pub(crate) struct CutawayLruCache {
    max_entries: usize,
    entries: Vec<((String, CutawayQuality), CutawayTextureAsset)>,
}

impl Default for CutawayLruCache {
    fn default() -> Self {
        Self::new(DEFAULT_CUTAWAY_CACHE_CAPACITY)
    }
}

impl CutawayLruCache {
    /// Creates a new bounded LRU cache retaining at most `max_entries` (clamped to `>= 1`).
    #[must_use]
    pub(crate) fn new(max_entries: usize) -> Self {
        let cap = max_entries.max(1);
        Self {
            max_entries: cap,
            entries: Vec::with_capacity(cap),
        }
    }

    /// Returns the number of cached entries currently stored.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` when the cache contains no entries.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns `true` if `(pic_or_sheet, quality)` is present without modifying recency order.
    #[must_use]
    pub(crate) fn contains_key(&self, pic_or_sheet: &str, quality: CutawayQuality) -> bool {
        self.entries
            .iter()
            .any(|((k, q), _)| *q == quality && k == pic_or_sheet)
    }

    /// Looks up `(pic_or_sheet, quality)`, promoting it to most-recently-used on hit.
    pub(crate) fn get(
        &mut self,
        pic_or_sheet: &str,
        quality: CutawayQuality,
    ) -> Option<&CutawayTextureAsset> {
        let pos = self
            .entries
            .iter()
            .position(|((k, q), _)| *q == quality && k == pic_or_sheet)?;
        let last = self.entries.len() - 1;
        if pos != last {
            let entry = self.entries.remove(pos);
            self.entries.push(entry);
        }
        self.entries.last().map(|(_, asset)| asset)
    }

    /// Inserts or replaces `key` as most-recently-used, evicting the least-recently-used entry
    /// when `self.entries.len()` exceeds `self.max_entries`.
    pub(crate) fn insert(&mut self, key: (String, CutawayQuality), value: CutawayTextureAsset) {
        if let Some(pos) = self.entries.iter().position(|(k, _)| *k == key) {
            self.entries.remove(pos);
        } else if self.entries.len() >= self.max_entries {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
    }

    /// Drops all cached cutaway textures immediately (e.g. on room transitions or menu exit).
    pub(crate) fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sh2_assets::DecodedTexture;

    fn dummy_asset(tag: u8) -> CutawayTextureAsset {
        CutawayTextureAsset {
            texture: DecodedTexture {
                width: 1,
                height: 1,
                rgba: vec![tag, tag, tag, 255],
            },
            original_width: 512,
            original_height: 512,
            is_sh2ee_hd: false,
        }
    }

    #[test]
    fn test_cutaway_lru_insert_hit_refresh_and_eviction() {
        let mut cache = CutawayLruCache::new(3);
        assert!(cache.is_empty());

        cache.insert(("a".into(), CutawayQuality::OriginalPs2), dummy_asset(1));
        cache.insert(("b".into(), CutawayQuality::OriginalPs2), dummy_asset(2));
        cache.insert(("c".into(), CutawayQuality::OriginalPs2), dummy_asset(3));
        assert_eq!(cache.len(), 3);

        // Accessing "a" refreshes it to MRU, making "b" the oldest (LRU).
        let a = cache
            .get("a", CutawayQuality::OriginalPs2)
            .expect("entry 'a' present");
        assert_eq!(a.texture.rgba[0], 1);

        // Inserting "d" evicts "b", preserving "c", "a", and "d".
        cache.insert(("d".into(), CutawayQuality::OriginalPs2), dummy_asset(4));
        assert_eq!(cache.len(), 3);
        assert!(!cache.contains_key("b", CutawayQuality::OriginalPs2));
        assert!(cache.contains_key("a", CutawayQuality::OriginalPs2));
        assert!(cache.contains_key("c", CutawayQuality::OriginalPs2));
        assert!(cache.contains_key("d", CutawayQuality::OriginalPs2));

        // Updating an existing key refreshes it without evicting another entry.
        cache.insert(("c".into(), CutawayQuality::OriginalPs2), dummy_asset(30));
        assert_eq!(cache.len(), 3);
        assert_eq!(
            cache
                .get("c", CutawayQuality::OriginalPs2)
                .expect("entry 'c' updated")
                .texture
                .rgba[0],
            30
        );

        // Next insert evicts "a" (now oldest after "c" was refreshed).
        cache.insert(("e".into(), CutawayQuality::OriginalPs2), dummy_asset(5));
        assert_eq!(cache.len(), 3);
        assert!(!cache.contains_key("a", CutawayQuality::OriginalPs2));
        assert!(cache.contains_key("d", CutawayQuality::OriginalPs2));
        assert!(cache.contains_key("c", CutawayQuality::OriginalPs2));
        assert!(cache.contains_key("e", CutawayQuality::OriginalPs2));

        // Clearing drops all entries.
        cache.clear();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_cutaway_lru_distinguishes_quality_tiers() {
        let mut cache = CutawayLruCache::new(2);
        cache.insert(
            ("out_p_incar".into(), CutawayQuality::OriginalPs2),
            dummy_asset(10),
        );
        cache.insert(
            ("out_p_incar".into(), CutawayQuality::Sh2eeHd),
            dummy_asset(20),
        );
        assert_eq!(cache.len(), 2);
        assert_eq!(
            cache
                .get("out_p_incar", CutawayQuality::OriginalPs2)
                .unwrap()
                .texture
                .rgba[0],
            10
        );
        assert_eq!(
            cache
                .get("out_p_incar", CutawayQuality::Sh2eeHd)
                .unwrap()
                .texture
                .rgba[0],
            20
        );
    }
}
