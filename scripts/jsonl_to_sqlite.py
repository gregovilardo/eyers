#!/usr/bin/env python3
"""
JSONL to SQLite Dictionary Converter

Converts Wiktionary JSONL dumps (from kaikki.org) into a SQLite database
for use with the Eyers application.

Usage:
    python jsonl_to_sqlite.py --en /path/to/en-wiktionary.jsonl \
                              --es /path/to/es-wiktionary.jsonl \
                              --output dictionary.db

You can provide either or both language files.
"""

import argparse
import json
import sqlite3
import sys
from pathlib import Path

# Only store translations for these language pairs
TARGET_TRANSLATIONS = {
    "en": ["es"],  # English entries: keep Spanish translations
    "es": ["en"],  # Spanish entries: keep English translations
}

BATCH_SIZE = 10000


def create_schema(conn: sqlite3.Connection) -> None:
    """Create the database schema with optimized indexes."""
    conn.executescript("""
        -- Main words table
        CREATE TABLE IF NOT EXISTS words (
            id INTEGER PRIMARY KEY,
            word TEXT NOT NULL,
            lang_code TEXT NOT NULL
        );

        -- Senses/definitions for each word
        CREATE TABLE IF NOT EXISTS senses (
            id INTEGER PRIMARY KEY,
            word_id INTEGER NOT NULL,
            pos TEXT,
            gloss TEXT NOT NULL,
            etymology_text TEXT,
            FOREIGN KEY (word_id) REFERENCES words(id)
        );

        -- Translations
        CREATE TABLE IF NOT EXISTS translations (
            id INTEGER PRIMARY KEY,
            sense_id INTEGER NOT NULL,
            target_lang TEXT NOT NULL,
            target_word TEXT NOT NULL,
            roman TEXT,
            FOREIGN KEY (sense_id) REFERENCES senses(id)
        );

        -- Indexes for fast lookups
        CREATE INDEX IF NOT EXISTS idx_words_lookup 
            ON words(word COLLATE NOCASE, lang_code);
        CREATE INDEX IF NOT EXISTS idx_senses_word 
            ON senses(word_id);
        CREATE INDEX IF NOT EXISTS idx_trans_lookup 
            ON translations(sense_id, target_lang);
    """)
    conn.commit()


def process_entry(entry: dict, lang_code: str) -> tuple:
    """
    Process a single JSONL entry and extract relevant data.
    
    Returns:
        (word, pos, senses_data, etymology_text)
        where senses_data is a list of (gloss, translations) tuples
    """
    word = entry.get("word", "")
    if not word:
        return None
    
    pos = entry.get("pos", "")
    
    # Get etymology - handle both formats
    etymology_text = None
    if "etymology_text" in entry:
        etymology_text = entry["etymology_text"]
    elif "etymology_texts" in entry and entry["etymology_texts"]:
        etymology_text = entry["etymology_texts"][0]
    
    # Get target languages for translations
    target_langs = TARGET_TRANSLATIONS.get(lang_code, [])
    
    # Extract senses and their translations
    senses_data = []
    
    # Get senses/definitions
    senses = entry.get("senses", [])
    for sense in senses:
        glosses = sense.get("glosses", [])
        if glosses:
            gloss = glosses[0]  # Take first gloss
            senses_data.append((gloss, []))
    
    # If no senses found, skip this entry
    if not senses_data:
        return None
    
    # Extract translations (they're at the entry level, not sense level)
    translations = entry.get("translations", [])
    relevant_translations = []
    
    for trans in translations:
        trans_lang = trans.get("lang_code") or trans.get("code", "")
        if trans_lang in target_langs:
            trans_word = trans.get("word", "")
            if trans_word:
                relevant_translations.append({
                    "target_lang": trans_lang,
                    "target_word": trans_word,
                    "roman": trans.get("roman"),
                })
    
    return (word, pos, senses_data, etymology_text, relevant_translations)


def process_file(
    conn: sqlite3.Connection,
    filepath: Path,
    lang_code: str,
    start_word_id: int,
    start_sense_id: int,
) -> tuple:
    """
    Process a JSONL file and insert entries into the database.
    
    Returns:
        (next_word_id, next_sense_id, entry_count)
    """
    word_id = start_word_id
    sense_id = start_sense_id
    entry_count = 0
    
    words_batch = []
    senses_batch = []
    trans_batch = []
    
    print(f"Processing {filepath.name} ({lang_code})...")
    
    with open(filepath, "r", encoding="utf-8") as f:
        for line_num, line in enumerate(f, 1):
            line = line.strip()
            if not line:
                continue
            
            try:
                entry = json.loads(line)
            except json.JSONDecodeError as e:
                print(f"  Warning: Skipping line {line_num} (invalid JSON): {e}")
                continue
            
            # Only process entries in the target language
            entry_lang = entry.get("lang_code", "")
            if entry_lang != lang_code:
                continue
            
            result = process_entry(entry, lang_code)
            if result is None:
                continue
            
            word, pos, senses_data, etymology_text, translations = result
            
            # Add word
            words_batch.append((word_id, word, lang_code))
            
            # Add senses
            first_sense_id = sense_id
            for gloss, _ in senses_data:
                senses_batch.append((
                    sense_id,
                    word_id,
                    pos,
                    gloss,
                    etymology_text,
                ))
                sense_id += 1
            
            # Add translations (link to first sense for simplicity)
            for trans in translations:
                trans_batch.append((
                    first_sense_id,
                    trans["target_lang"],
                    trans["target_word"],
                    trans["roman"],
                ))
            
            word_id += 1
            entry_count += 1
            
            # Batch insert
            if entry_count % BATCH_SIZE == 0:
                flush_batches(conn, words_batch, senses_batch, trans_batch)
                words_batch = []
                senses_batch = []
                trans_batch = []
                print(f"  Processed {entry_count:,} entries...")
    
    # Final flush
    if words_batch or senses_batch or trans_batch:
        flush_batches(conn, words_batch, senses_batch, trans_batch)
    
    print(f"  Completed: {entry_count:,} entries")
    return (word_id, sense_id, entry_count)


def flush_batches(
    conn: sqlite3.Connection,
    words: list,
    senses: list,
    translations: list,
) -> None:
    """Insert batched data into the database."""
    cursor = conn.cursor()
    
    if words:
        cursor.executemany(
            "INSERT INTO words (id, word, lang_code) VALUES (?, ?, ?)",
            words
        )
    
    if senses:
        cursor.executemany(
            "INSERT INTO senses (id, word_id, pos, gloss, etymology_text) "
            "VALUES (?, ?, ?, ?, ?)",
            senses
        )
    
    if translations:
        cursor.executemany(
            "INSERT INTO translations (sense_id, target_lang, target_word, roman) "
            "VALUES (?, ?, ?, ?)",
            translations
        )
    
    conn.commit()


def main():
    parser = argparse.ArgumentParser(
        description="Convert Wiktionary JSONL files to SQLite database"
    )
    parser.add_argument(
        "--en",
        type=Path,
        help="Path to English Wiktionary JSONL file",
    )
    parser.add_argument(
        "--es",
        type=Path,
        help="Path to Spanish Wiktionary JSONL file",
    )
    parser.add_argument(
        "--output", "-o",
        type=Path,
        default=Path("dictionary.db"),
        help="Output SQLite database path (default: dictionary.db)",
    )
    
    args = parser.parse_args()
    
    if not args.en and not args.es:
        parser.error("At least one of --en or --es must be provided")
    
    # Validate input files exist
    for lang, path in [("en", args.en), ("es", args.es)]:
        if path and not path.exists():
            print(f"Error: {lang} file not found: {path}", file=sys.stderr)
            sys.exit(1)
    
    # Remove existing database if it exists
    if args.output.exists():
        print(f"Removing existing database: {args.output}")
        args.output.unlink()
    
    # Create database and schema
    print(f"Creating database: {args.output}")
    conn = sqlite3.connect(args.output)
    
    # Optimize for bulk inserts
    conn.execute("PRAGMA journal_mode = WAL")
    conn.execute("PRAGMA synchronous = NORMAL")
    conn.execute("PRAGMA cache_size = -64000")  # 64MB cache
    
    create_schema(conn)
    
    word_id = 1
    sense_id = 1
    total_entries = 0
    
    # Process English file
    if args.en:
        word_id, sense_id, count = process_file(
            conn, args.en, "en", word_id, sense_id
        )
        total_entries += count
    
    # Process Spanish file
    if args.es:
        word_id, sense_id, count = process_file(
            conn, args.es, "es", word_id, sense_id
        )
        total_entries += count
    
    # Optimize database after bulk inserts
    print("Optimizing database...")
    conn.execute("PRAGMA optimize")
    conn.execute("VACUUM")
    conn.close()
    
    # Report final size
    size_mb = args.output.stat().st_size / (1024 * 1024)
    print(f"\nDone! Created {args.output}")
    print(f"  Total entries: {total_entries:,}")
    print(f"  Database size: {size_mb:.1f} MB")


if __name__ == "__main__":
    main()
