use gtk::glib;
use rusqlite::{Connection, OpenFlags};
use std::path::PathBuf;

/// The language mode for dictionary lookups.
#[derive(Debug, Clone, Copy, Default)]
pub enum Language {
    #[default]
    English,
    Spanish,
}

impl Language {
    /// Returns the ISO 639-1 code for this language.
    pub fn code(&self) -> &'static str {
        match self {
            Language::English => "en",
            Language::Spanish => "es",
        }
    }

    /// Returns the target language code for translations.
    pub fn translation_target(&self) -> &'static str {
        match self {
            Language::English => "es",
            Language::Spanish => "en",
        }
    }
}

/// A single sense (definition) of a word.
#[derive(Debug)]
pub struct Sense {
    pub pos: String,
    pub gloss: String,
    pub etymology: Option<String>,
    pub translations: Vec<Translation>,
}

/// A translation of a sense to another language.
#[derive(Debug)]
pub struct Translation {
    pub word: String,
    pub romanization: Option<String>,
}

/// Result of a dictionary lookup.
#[derive(Debug)]
pub struct LookupResult {
    pub word: String,
    pub senses: Vec<Sense>,
}

/// Returns the path to the dictionary database.
fn get_db_path() -> Option<PathBuf> {
    dirs::data_dir().map(|p| p.join("eyers").join("dictionary.db"))
}

/// Opens a read-only connection to the dictionary database.
fn open_db() -> Option<Connection> {
    let path = get_db_path()?;
    if !path.exists() {
        return None;
    }
    Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY).ok()
}

/// Looks up a word in the dictionary.
pub fn lookup(word: &str, lang: Language) -> Option<LookupResult> {
    let conn = open_db()?;
    let lang_code = lang.code();
    let target_lang = lang.translation_target();

    // Find the word in the words table
    let word_id: i64 = conn
        .query_row(
            "SELECT id FROM words WHERE word = ?1 COLLATE NOCASE AND lang_code = ?2 LIMIT 1",
            [word, lang_code],
            |row| row.get(0),
        )
        .ok()?;

    // Get all senses for this word
    let mut sense_stmt = conn
        .prepare("SELECT id, pos, gloss, etymology_text FROM senses WHERE word_id = ?1 ORDER BY id")
        .ok()?;

    let senses: Vec<Sense> = sense_stmt
        .query_map([word_id], |row| {
            let sense_id: i64 = row.get(0)?;
            let pos: String = row.get(1)?;
            let gloss: String = row.get(2)?;
            let etymology: Option<String> = row.get(3)?;
            Ok((sense_id, pos, gloss, etymology))
        })
        .ok()?
        .filter_map(|r| r.ok())
        .map(|(sense_id, pos, gloss, etymology)| {
            // Get translations for this sense
            let translations = get_translations(&conn, sense_id, target_lang);
            Sense {
                pos,
                gloss,
                etymology,
                translations,
            }
        })
        .collect();

    if senses.is_empty() {
        return None;
    }

    Some(LookupResult {
        word: word.to_string(),
        senses,
    })
}

/// Gets translations for a sense.
fn get_translations(conn: &Connection, sense_id: i64, target_lang: &str) -> Vec<Translation> {
    let mut stmt = match conn.prepare(
        "SELECT target_word, roman FROM translations WHERE sense_id = ?1 AND target_lang = ?2",
    ) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    stmt.query_map(rusqlite::params![sense_id, target_lang], |row| {
        let word: String = row.get(0)?;
        let romanization: Option<String> = row.get(1)?;
        Ok(Translation { word, romanization })
    })
    .ok()
    .map(|iter| iter.filter_map(|r| r.ok()).collect())
    .unwrap_or_default()
}

/// Fetches and formats a definition for display.
/// This is the main entry point called by the UI.
pub fn fetch_definition(lookup_word: &str, display_word: &str, lang: Language) -> Option<String> {
    let result = lookup(lookup_word, lang)?;
    format_result(&result, display_word)
}

/// Formats a lookup result as Pango markup for display.
fn format_result(result: &LookupResult, display_word: &str) -> Option<String> {
    let mut output = String::new();
    let escaped_display = glib::markup_escape_text(display_word);

    output.push_str(&format!(
        "<span size='large' weight='bold'>{}</span>\n\n",
        escaped_display
    ));

    // Group senses by part of speech
    let mut current_pos: Option<&str> = None;
    let mut def_num = 0;

    for sense in &result.senses {
        // Print POS header if it changed
        if current_pos != Some(&sense.pos) {
            if current_pos.is_some() {
                output.push('\n');
            }
            let escaped_pos = glib::markup_escape_text(&sense.pos);
            output.push_str(&format!("<b><i>{}</i></b>\n", escaped_pos));
            current_pos = Some(&sense.pos);
            def_num = 0;
        }

        def_num += 1;
        let escaped_gloss = glib::markup_escape_text(&sense.gloss);
        output.push_str(&format!(" {}. {}\n", def_num, escaped_gloss));

        // Add translations if present
        if !sense.translations.is_empty() {
            let trans_str: String = sense
                .translations
                .iter()
                .map(|t| {
                    let escaped = glib::markup_escape_text(&t.word);
                    if let Some(ref roman) = t.romanization {
                        format!("{} ({})", escaped, glib::markup_escape_text(roman))
                    } else {
                        escaped.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            output.push_str(&format!(
                "    <span color='#666666'><small>{}</small></span>\n",
                trans_str
            ));
        }
    }

    let final_output = output.trim().to_string();
    if final_output.is_empty() {
        None
    } else {
        Some(final_output)
    }
}
