use crate::modes::WordCursor;
use crate::services::annotations::Annotation;

#[derive(Clone, Debug, Default)]
pub struct AnnotationObject {
    annotation: Annotation,
}

impl AnnotationObject {
    pub fn new(annotation: Annotation) -> Self {
        Self { annotation }
    }

    pub fn id(&self) -> i64 {
        self.annotation.id
    }

    pub fn start_page(&self) -> usize {
        self.annotation.start_page
    }

    pub fn start_word(&self) -> usize {
        self.annotation.start_word
    }

    pub fn selected_text(&self) -> String {
        self.annotation.selected_text.clone()
    }

    pub fn note(&self) -> String {
        self.annotation.note.clone()
    }

    pub fn get_start_word_cursor(&self) -> WordCursor {
        self.annotation.get_start_word_cursor()
    }
}

impl PartialEq for AnnotationObject {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

pub fn compare_annotations(a: &AnnotationObject, b: &AnnotationObject) -> std::cmp::Ordering {
    let page_cmp = a.start_page().cmp(&b.start_page());
    if page_cmp != std::cmp::Ordering::Equal {
        return page_cmp;
    }
    a.start_word().cmp(&b.start_word())
}
