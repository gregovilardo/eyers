use pdfium_render::prelude::*;

#[derive(Debug, Clone)]
pub struct BookmarkEntry {
    pub title: String,
    pub page_index: u32,
    pub children: Vec<BookmarkEntry>,
    pub depth: usize,
}

pub fn extract_bookmarks(document: &PdfDocument<'_>) -> Vec<BookmarkEntry> {
    document
        .bookmarks()
        .iter()
        .filter(|bookmark| bookmark.parent().is_none())
        .filter_map(|bookmark| process_bookmark(&bookmark, 0))
        .collect()
}

fn process_bookmark(bookmark: &PdfBookmark, depth: usize) -> Option<BookmarkEntry> {
    let title = bookmark
        .title()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "Untitled".to_string());

    let page_index = bookmark
        .destination()
        .and_then(|dest| dest.page_index().ok())
        .unwrap_or(0);

    let mut children = Vec::new();

    let mut child = bookmark.first_child();

    while let Some(c) = child {
        if let Some(child_entry) = process_bookmark(&c, depth + 1) {
            children.push(child_entry);
        }
        child = c.next_sibling();
    }

    Some(BookmarkEntry {
        title,
        page_index,
        children,
        depth,
    })
}
