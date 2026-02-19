use gtk::glib;
use rusqlite::{Connection, OpenFlags, params};
use std::{cmp::Ordering, path::PathBuf};

use crate::modes::WordCursor;

pub type AnnotationId = i64;

/// Represents an annotation on a PDF document
#[derive(Debug, Clone, Default)]
pub struct Annotation {
    pub id: AnnotationId,
    pub pdf_path: String,
    pub start_page: usize,
    pub start_word: usize,
    pub end_page: usize,
    pub end_word: usize,
    pub selected_text: String,
    pub note: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Error type for annotation operations
#[derive(Debug)]
pub enum AnnotationError {
    DatabaseError(String),
    NotFound,
}

impl Annotation {
    pub fn get_start_word_cursor(&self) -> WordCursor {
        WordCursor::new(self.start_page, self.start_word)
    }
    pub fn get_id(&self) -> AnnotationId {
        self.id
    }
}

impl PartialEq for Annotation {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl PartialOrd for Annotation {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        let page_cmp = self.start_page.cmp(&other.start_page);
        if page_cmp != Ordering::Equal {
            return Some(page_cmp);
        }
        Some(self.start_word.cmp(&other.start_word))
    }
}

impl std::fmt::Display for AnnotationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnnotationError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
            AnnotationError::NotFound => write!(f, "Annotation not found"),
        }
    }
}

impl std::error::Error for AnnotationError {}

impl From<rusqlite::Error> for AnnotationError {
    fn from(err: rusqlite::Error) -> Self {
        AnnotationError::DatabaseError(err.to_string())
    }
}

/// Returns the path to the annotations database
fn get_db_path() -> Option<PathBuf> {
    dirs::data_dir().map(|p| p.join("eyers").join("annotations.db"))
}

/// Opens a connection to the annotations database, creating it if necessary
fn open_db() -> Result<Connection, AnnotationError> {
    let path = get_db_path().ok_or_else(|| {
        AnnotationError::DatabaseError("Could not determine data directory".to_string())
    })?;

    // Create parent directory if it doesn't exist
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            AnnotationError::DatabaseError(format!("Could not create data directory: {}", e))
        })?;
    }

    let conn = Connection::open_with_flags(
        &path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
    )?;

    // Initialize the schema if needed
    conn.execute(
        "CREATE TABLE IF NOT EXISTS annotations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            pdf_path TEXT NOT NULL,
            start_page INTEGER NOT NULL,
            start_word INTEGER NOT NULL,
            end_page INTEGER NOT NULL,
            end_word INTEGER NOT NULL,
            selected_text TEXT NOT NULL,
            note TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        )",
        [],
    )?;

    // Create index for faster lookups by PDF path
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_annotations_pdf_path ON annotations(pdf_path)",
        [],
    )?;

    Ok(conn)
}

/// Save a new annotation to the database
pub fn save_annotation(
    pdf_path: &str,
    start_page: usize,
    start_word: usize,
    end_page: usize,
    end_word: usize,
    selected_text: &str,
    note: &str,
) -> Result<i64, AnnotationError> {
    let conn = open_db()?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    conn.execute(
        "INSERT INTO annotations (pdf_path, start_page, start_word, end_page, end_word, selected_text, note, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            pdf_path,
            start_page as i64,
            start_word as i64,
            end_page as i64,
            end_word as i64,
            selected_text,
            note,
            now,
            now
        ],
    )?;

    Ok(conn.last_insert_rowid())
}

/// Update an existing annotation's note and selection range
pub fn update_annotation(
    id: i64,
    start_page: usize,
    start_word: usize,
    end_page: usize,
    end_word: usize,
    selected_text: &str,
    note: &str,
) -> Result<(), AnnotationError> {
    let conn = open_db()?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let rows_affected = conn.execute(
        "UPDATE annotations SET start_page = ?1, start_word = ?2, end_page = ?3, end_word = ?4, selected_text = ?5, note = ?6, updated_at = ?7 WHERE id = ?8",
        params![
            start_page as i64,
            start_word as i64,
            end_page as i64,
            end_word as i64,
            selected_text,
            note,
            now,
            id
        ],
    )?;

    if rows_affected == 0 {
        return Err(AnnotationError::NotFound);
    }

    Ok(())
}

/// Delete an annotation by ID
pub fn delete_annotation(id: i64) -> Result<(), AnnotationError> {
    let conn = open_db()?;

    let rows_affected = conn.execute("DELETE FROM annotations WHERE id = ?1", params![id])?;

    if rows_affected == 0 {
        return Err(AnnotationError::NotFound);
    }

    Ok(())
}

/// Load all annotations for a specific PDF file
pub fn load_annotations_for_pdf(pdf_path: &str) -> Result<Vec<Annotation>, AnnotationError> {
    let conn = open_db()?;

    let mut stmt = conn.prepare(
        "SELECT id, pdf_path, start_page, start_word, end_page, end_word, selected_text, note, created_at, updated_at
         FROM annotations WHERE pdf_path = ?1 ORDER BY start_page, start_word",
    )?;

    let annotations = stmt
        .query_map(params![pdf_path], |row| {
            Ok(Annotation {
                id: row.get(0)?,
                pdf_path: row.get(1)?,
                start_page: row.get::<_, i64>(2)? as usize,
                start_word: row.get::<_, i64>(3)? as usize,
                end_page: row.get::<_, i64>(4)? as usize,
                end_word: row.get::<_, i64>(5)? as usize,
                selected_text: row.get(6)?,
                note: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(annotations)
}

/// Get a single annotation by ID
pub fn get_annotation(id: i64) -> Result<Annotation, AnnotationError> {
    let conn = open_db()?;

    conn.query_row(
        "SELECT id, pdf_path, start_page, start_word, end_page, end_word, selected_text, note, created_at, updated_at
         FROM annotations WHERE id = ?1",
        params![id],
        |row| {
            Ok(Annotation {
                id: row.get(0)?,
                pdf_path: row.get(1)?,
                start_page: row.get::<_, i64>(2)? as usize,
                start_word: row.get::<_, i64>(3)? as usize,
                end_page: row.get::<_, i64>(4)? as usize,
                end_word: row.get::<_, i64>(5)? as usize,
                selected_text: row.get(6)?,
                note: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        },
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AnnotationError::NotFound,
        _ => AnnotationError::DatabaseError(e.to_string()),
    })
}

pub fn find_prev_annotation_at_position(
    pdf_path: &str,
    page_index: usize,
    word_index: usize,
) -> Result<Option<Annotation>, AnnotationError> {
    let annotations = load_annotations_for_pdf(pdf_path)?;

    for ann in annotations.into_iter().rev() {
        let pos = (page_index, word_index);
        let end = (ann.end_page, ann.end_word);
        // If annotations come ordered from start to finish it would work find
        if pos > end {
            return Ok(Some(ann));
        }
    }

    Ok(None)
}

pub fn find_next_annotation_at_position(
    pdf_path: &str,
    page_index: usize,
    word_index: usize,
) -> Result<Option<Annotation>, AnnotationError> {
    let annotations = load_annotations_for_pdf(pdf_path)?;

    for ann in annotations {
        let pos = (page_index, word_index);
        let start = (ann.start_page, ann.start_word);
        // If annotations come ordered from start to finish it would work find
        if pos < start {
            return Ok(Some(ann));
        }
    }

    Ok(None)
}

/// Find an annotation that contains a specific word position
/// Returns the annotation if the word at (page_index, word_index) falls within any annotation's range
pub fn find_annotation_at_position(
    pdf_path: &str,
    page_index: usize,
    word_index: usize,
) -> Result<Option<Annotation>, AnnotationError> {
    let annotations = load_annotations_for_pdf(pdf_path)?;

    for ann in annotations {
        if is_position_in_annotation(&ann, page_index, word_index) {
            return Ok(Some(ann));
        }
    }

    Ok(None)
}

/// Find annotations that overlap with a given selection range
pub fn find_overlapping_annotations(
    pdf_path: &str,
    start_page: usize,
    start_word: usize,
    end_page: usize,
    end_word: usize,
) -> Result<Vec<Annotation>, AnnotationError> {
    let annotations = load_annotations_for_pdf(pdf_path)?;

    let overlapping: Vec<Annotation> = annotations
        .into_iter()
        .filter(|ann| ranges_overlap(ann, start_page, start_word, end_page, end_word))
        .collect();

    Ok(overlapping)
}

/// Check if a position is within an annotation's range
fn is_position_in_annotation(ann: &Annotation, page_index: usize, word_index: usize) -> bool {
    let pos = (page_index, word_index);
    let start = (ann.start_page, ann.start_word);
    let end = (ann.end_page, ann.end_word);

    pos >= start && pos <= end
}

/// Check if two ranges overlap
fn ranges_overlap(
    ann: &Annotation,
    start_page: usize,
    start_word: usize,
    end_page: usize,
    end_word: usize,
) -> bool {
    let ann_start = (ann.start_page, ann.start_word);
    let ann_end = (ann.end_page, ann.end_word);
    let sel_start = (start_page, start_word);
    let sel_end = (end_page, end_word);

    // Two ranges overlap if one starts before the other ends and vice versa
    ann_start <= sel_end && sel_start <= ann_end
}

/// Export annotations for a PDF to markdown format
/// Each annotation is formatted as:
/// > "highlighted text" (Page X)
///
/// User's note
pub fn export_to_markdown(pdf_path: &str, pdf_name: &str) -> Result<String, AnnotationError> {
    let annotations = load_annotations_for_pdf(pdf_path)?;

    if annotations.is_empty() {
        return Ok(format!(
            "# Annotations for {}\n\nNo annotations found.\n",
            pdf_name
        ));
    }

    let mut output = format!("# Annotations for {}\n\n", pdf_name);

    for ann in annotations {
        // Page number is 1-indexed for display
        let page_num = ann.start_page + 1;

        // Quote the highlighted text
        output.push_str(&format!(
            "> **\"{}\"** (Page {})\n\n",
            ann.selected_text, page_num
        ));

        // Add the user's note
        if !ann.note.is_empty() {
            output.push_str(&ann.note);
            output.push_str("\n\n");
        }

        output.push_str("---\n\n");
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_position_in_annotation() {
        let ann = Annotation {
            id: 1,
            pdf_path: "test.pdf".to_string(),
            start_page: 0,
            start_word: 5,
            end_page: 0,
            end_word: 10,
            selected_text: "test".to_string(),
            note: "note".to_string(),
            created_at: 0,
            updated_at: 0,
        };

        // Inside
        assert!(is_position_in_annotation(&ann, 0, 7));
        // At start
        assert!(is_position_in_annotation(&ann, 0, 5));
        // At end
        assert!(is_position_in_annotation(&ann, 0, 10));
        // Before
        assert!(!is_position_in_annotation(&ann, 0, 4));
        // After
        assert!(!is_position_in_annotation(&ann, 0, 11));
    }

    #[test]
    fn test_ranges_overlap() {
        let ann = Annotation {
            id: 1,
            pdf_path: "test.pdf".to_string(),
            start_page: 0,
            start_word: 5,
            end_page: 0,
            end_word: 10,
            selected_text: "test".to_string(),
            note: "note".to_string(),
            created_at: 0,
            updated_at: 0,
        };

        // Partial overlap
        assert!(ranges_overlap(&ann, 0, 8, 0, 15));
        // Full overlap (selection contains annotation)
        assert!(ranges_overlap(&ann, 0, 0, 0, 20));
        // No overlap (before)
        assert!(!ranges_overlap(&ann, 0, 0, 0, 4));
        // No overlap (after)
        assert!(!ranges_overlap(&ann, 0, 11, 0, 15));
    }
}
