use gtk::glib;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct WordEntry {
    #[serde(default)]
    pub meanings: Vec<Meaning>,
}

#[derive(Deserialize, Debug)]
pub struct Meaning {
    #[serde(rename = "partOfSpeech")]
    pub part_of_speech: String,
    #[serde(default)]
    pub definitions: Vec<Definition>,
}

#[derive(Deserialize, Debug)]
pub struct Definition {
    pub definition: String,
}

pub fn fetch_definition(lookup_word: &str, display_word: &str) -> Option<String> {
    let url = format!(
        "https://api.dictionaryapi.dev/api/v2/entries/en/{}",
        lookup_word
    );

    let response = reqwest::blocking::get(&url).ok()?;
    if !response.status().is_success() {
        return None;
    }

    let entries: Vec<WordEntry> = response.json().ok()?;
    let entry = entries.first()?;

    format_definition(entry, display_word)
}

fn format_definition(entry: &WordEntry, display_word: &str) -> Option<String> {
    let mut output = String::new();
    let escaped_display = glib::markup_escape_text(display_word);

    output.push_str(&format!(
        "<span size='large' weight='bold'>{}</span>\n\n",
        escaped_display
    ));

    for meaning in &entry.meanings {
        format_meaning(&mut output, meaning);
    }

    let final_output = output.trim().to_string();
    if final_output.is_empty() {
        None
    } else {
        Some(final_output)
    }
}

fn format_meaning(output: &mut String, meaning: &Meaning) {
    let escaped_pos = glib::markup_escape_text(&meaning.part_of_speech);
    output.push_str(&format!("<b><i>{}</i></b>\n", escaped_pos));

    for (i, def) in meaning.definitions.iter().enumerate() {
        let escaped_def = glib::markup_escape_text(&def.definition);
        output.push_str(&format!(" {}. {}\n", i + 1, escaped_def));
    }
    output.push('\n');
}
