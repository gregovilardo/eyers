use std::collections::HashMap;

use pdfium_render::prelude::*;

use crate::text_map::page_text_map::PageTextMap;

/// Lazy cache for PageTextMap instances across a PDF document
#[derive(Debug)]
pub struct TextMapCache {
    /// Cached text maps by page index
    maps: HashMap<usize, PageTextMap>,
    /// Total number of pages in the document
    page_count: usize,
}

impl TextMapCache {
    /// Create a new empty cache for a document
    pub fn new(page_count: usize) -> Self {
        Self {
            maps: HashMap::new(),
            page_count,
        }
    }

    /// Get or build the PageTextMap for a specific page
    /// Returns None if the page doesn't exist or text extraction fails
    pub fn get_or_build(
        &mut self,
        page_index: usize,
        document: &PdfDocument,
    ) -> Option<&PageTextMap> {
        if page_index >= self.page_count {
            return None;
        }

        // Build if not cached
        if !self.maps.contains_key(&page_index) {
            let pages = document.pages();
            let page = pages.get(page_index as u16).ok()?;
            let text_map = PageTextMap::build_from_page(&page, page_index)?;
            self.maps.insert(page_index, text_map);
        }

        self.maps.get(&page_index)
    }

    /// Get a cached PageTextMap without building
    /// Returns None if not yet cached
    pub fn get(&self, page_index: usize) -> Option<&PageTextMap> {
        self.maps.get(&page_index)
    }

    /// Check if a page's text map is already cached
    pub fn is_cached(&self, page_index: usize) -> bool {
        self.maps.contains_key(&page_index)
    }

    /// Clear all cached data
    pub fn clear(&mut self) {
        self.maps.clear();
    }

    /// Get the total page count
    pub fn page_count(&self) -> usize {
        self.page_count
    }

    /// Pre-build text maps for a range of pages (useful for background loading)
    pub fn prebuild_range(&mut self, start: usize, end: usize, document: &PdfDocument) {
        for page_index in start..end.min(self.page_count) {
            if !self.is_cached(page_index) {
                if let Ok(page) = document.pages().get(page_index as u16) {
                    if let Some(text_map) = PageTextMap::build_from_page(&page, page_index) {
                        self.maps.insert(page_index, text_map);
                    }
                }
            }
        }
    }
}
