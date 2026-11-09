//! Pages of the preview: the page images of the analysis (rotated and deskewed) plus the remaining ones, which are
//! loaded one by one while paging. The analysis reads only the first pages (`max_pages`), the preview shows all of
//! them. The pages of the analysis and those around the shown one stay loaded; at 1400 px a page takes a good
//! 5 MB of graphics memory.

use eframe::egui::TextureHandle;

/// This many pages before and after the shown one stay loaded while the document is shown.
const KEEP_AROUND: usize = 4;
/// A request loads at most this many pages: the shown one and the ones for paging on.
const BATCH: usize = 3;

pub enum Slot {
    Missing,
    Loaded(TextureHandle),
    /// Could not be loaded; the error is shown in place of the page.
    Failed(String),
}

#[derive(Default)]
pub struct PageCache {
    /// One per page of the document; empty while the page count is unknown.
    slots: Vec<Slot>,
    /// This many pages at the start come from the analysis; they always stay loaded.
    analysed: usize,
    /// A request to the preview thread is running. No new one is sent until it returns: otherwise fast paging
    /// would pile up pages nobody looks at any more.
    pub loading: bool,
    /// The document could not be opened. No further request is sent until it is shown again; otherwise there
    /// would be one every frame.
    unopened: bool,
}

impl PageCache {
    /// Page count of the document; 0 while it is unknown.
    pub fn count(&self) -> usize {
        self.slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    pub fn get(&self, page: usize) -> Option<&Slot> {
        self.slots.get(page)
    }

    /// Any loaded page, for the size of one that is still loading.
    pub fn any_loaded(&self, near: usize) -> Option<&TextureHandle> {
        let loaded = |i: usize| match self.slots.get(i) {
            Some(Slot::Loaded(tex)) => Some(tex),
            _ => None,
        };
        (0..self.slots.len()).map(|d| near.saturating_sub(d)).chain(near..self.slots.len()).find_map(loaded)
    }

    /// Page count as reported by the document (0 means unknown, change nothing).
    pub fn set_count(&mut self, count: usize) {
        if count > 0 && count != self.slots.len() {
            self.slots.resize_with(count, || Slot::Missing);
            self.analysed = self.analysed.min(count);
        }
    }

    /// The pages of the analysis replace the first pages of the preview; the others stay as they are.
    pub fn set_analysed(&mut self, textures: Vec<TextureHandle>, page_count: usize) {
        self.set_count(page_count.max(textures.len()));
        self.analysed = textures.len();
        for (slot, tex) in self.slots.iter_mut().zip(textures) {
            *slot = Slot::Loaded(tex);
        }
    }

    /// Answer of the preview thread: the page count (0 if the document could not be opened) and the pages.
    pub fn receive(&mut self, page_count: usize, pages: Vec<(usize, Slot)>) {
        self.loading = false;
        self.unopened = page_count == 0;
        self.set_count(page_count);
        for (i, slot) in pages {
            self.set_page(i, slot);
        }
    }

    /// A page loaded later. A page of the analysis stays: it is already rotated and deskewed, and the request may
    /// still have been on its way when the analysis finished.
    pub fn set_page(&mut self, page: usize, slot: Slot) {
        if page >= self.analysed
            && let Some(s) = self.slots.get_mut(page)
        {
            *s = slot;
        }
    }

    /// Which pages to load next: the shown one, the two after it and the one before, as far as they are missing.
    /// Without a known page count the first ones; the answer brings the page count along.
    pub fn wanted(&self, current: usize) -> Vec<usize> {
        if self.loading || self.unopened {
            return Vec::new();
        }
        if self.slots.is_empty() {
            return (current..current + BATCH).collect();
        }
        let missing = |i: &usize| matches!(self.slots.get(*i), Some(Slot::Missing));
        [Some(current), Some(current + 1), Some(current + 2), current.checked_sub(1)]
            .into_iter()
            .flatten()
            .filter(missing)
            .take(BATCH)
            .collect()
    }

    /// Frees loaded pages that are far from the shown one. For a document that is not shown at the moment, only
    /// its last shown page stays. An error counts as free: the page is tried again next time, and so is a
    /// document that could not be opened.
    pub fn trim(&mut self, current: usize, shown: bool) {
        let keep = if shown { KEEP_AROUND } else { 0 };
        self.unopened &= shown;
        for (i, slot) in self.slots.iter_mut().enumerate().skip(self.analysed) {
            if i.abs_diff(current) > keep {
                *slot = Slot::Missing;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{ColorImage, Context, TextureOptions};

    fn texture(ctx: &Context, name: &str) -> TextureHandle {
        ctx.load_texture(name, ColorImage::filled([2, 3], eframe::egui::Color32::WHITE), TextureOptions::LINEAR)
    }

    fn loaded(cache: &PageCache) -> Vec<usize> {
        (0..cache.count()).filter(|&i| matches!(cache.get(i), Some(Slot::Loaded(_)))).collect()
    }

    #[test]
    fn all_pages_reachable_beyond_the_analysis() {
        let ctx = Context::default();
        let mut cache = PageCache::default();
        // No page count yet: request the first pages.
        assert_eq!(cache.wanted(0), [0, 1, 2]);
        cache.set_count(20);
        for i in 0..3 {
            cache.set_page(i, Slot::Loaded(texture(&ctx, "vorschau")));
        }
        // The analysis reads three pages; the preview keeps all 20.
        cache.set_analysed((0..3).map(|_| texture(&ctx, "analyse")).collect(), 20);
        assert_eq!((cache.count(), cache.analysed), (20, 3));

        // Paging to the last page: only the pages around the shown one are ever missing.
        assert_eq!(cache.wanted(2), [3, 4]);
        assert_eq!(cache.wanted(19), [19, 18]);
        cache.loading = true;
        assert!(cache.wanted(19).is_empty(), "only one request at a time");
        cache.loading = false;
        for i in [18, 19] {
            cache.set_page(i, Slot::Loaded(texture(&ctx, "nachgeladen")));
        }
        cache.set_page(17, Slot::Failed("kaputt".into()));
        assert!(cache.wanted(19).is_empty());
        assert_eq!(loaded(&cache), [0, 1, 2, 18, 19]);

        // A late preview page does not overwrite a page of the analysis.
        cache.set_page(1, Slot::Failed("zu spät".into()));
        assert!(matches!(cache.get(1), Some(Slot::Loaded(_))));
        // A later analysis does not cut the preview down to its three pages.
        cache.set_analysed((0..3).map(|_| texture(&ctx, "neu")).collect(), 20);
        assert_eq!(cache.count(), 20);
        assert_eq!(loaded(&cache), [0, 1, 2, 18, 19]);
    }

    #[test]
    fn far_pages_are_freed() {
        let ctx = Context::default();
        let mut cache = PageCache::default();
        cache.set_analysed(vec![texture(&ctx, "analyse")], 100);
        for i in 40..=60 {
            cache.set_page(i, Slot::Loaded(texture(&ctx, "nachgeladen")));
        }
        cache.trim(50, true);
        assert_eq!(loaded(&cache), [0, 46, 47, 48, 49, 50, 51, 52, 53, 54]);
        // Not shown: only the last shown page and the one of the analysis.
        cache.trim(50, false);
        assert_eq!(loaded(&cache), [0, 50]);
        // Errors are tried again next time.
        cache.set_page(70, Slot::Failed("weg".into()));
        cache.trim(50, true);
        assert!(matches!(cache.get(70), Some(Slot::Missing)));
        assert_eq!(cache.any_loaded(70).map(|t| t.name()), Some("nachgeladen".to_string()));
    }

    #[test]
    fn unopened_document_is_not_asked_again_and_again() {
        let mut cache = PageCache { loading: true, ..Default::default() };
        cache.receive(0, vec![(0, Slot::Failed("Datei fehlt".into()))]);
        assert!(!cache.loading && cache.is_empty());
        assert!(cache.wanted(0).is_empty());
        cache.trim(0, true);
        assert!(cache.wanted(0).is_empty());
        // A new attempt only once it is shown again.
        cache.trim(0, false);
        assert_eq!(cache.wanted(0), [0, 1, 2]);
    }
}
