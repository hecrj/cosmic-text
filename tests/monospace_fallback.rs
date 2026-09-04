//! Regression tests for
//! <https://github.com/pop-os/cosmic-text/issues/518>: shaping with the
//! generic `Family::Monospace` used to rescan every face in the database
//! for every word (including every whitespace-only word), making it 10-20x
//! slower than shaping with an explicit `Family::Name`.

use cosmic_text::{fontdb, Attrs, Buffer, Family, FontSystem, Metrics, Shaping};
use std::path::PathBuf;

/// Build a font database containing only the repo test fonts, i.e. no
/// "Noto Sans Mono" - cosmic-text's default monospace family - simulating
/// e.g. a stock macOS system.
fn font_system() -> FontSystem {
    let repo_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let fonts_path = PathBuf::from(&repo_dir).join("fonts");

    let mut db = fontdb::Database::new();
    db.load_font_data(std::fs::read(fonts_path.join("FiraMono-Medium.ttf")).unwrap());
    db.load_font_data(std::fs::read(fonts_path.join("Inter-Regular.ttf")).unwrap());
    // Mirror the defaults set by `FontSystem` for the generic families.
    db.set_monospace_family("Noto Sans Mono");
    db.set_sans_serif_family("Open Sans");
    db.set_serif_family("DejaVu Serif");

    FontSystem::new_with_locale_and_db("en-US".to_string(), db)
}

/// Whitespace-heavy text: every whitespace character is shaped as its own
/// word, so each one used to trigger a full monospace fallback scan.
const SPACED_TEXT: &str = "Label                    value";

/// Shaping with the generic `Family::Monospace` must fall back to an
/// installed monospaced face for every word - including the whitespace
/// words - even when the default monospace family is not installed.
#[test]
fn monospace_family_fallback_default_missing() {
    let mut font_system = font_system();
    let metrics = Metrics::new(14.0, 20.0);
    let mut buffer = Buffer::new(&mut font_system, metrics);

    buffer.set_text(
        SPACED_TEXT,
        &Attrs::new().family(Family::Monospace),
        Shaping::Advanced,
        None,
    );
    buffer.shape_until_scroll(&mut font_system, false);

    let glyph_font_ids: Vec<fontdb::ID> = buffer
        .layout_runs()
        .flat_map(|run| run.glyphs.iter().map(|g| g.font_id))
        .collect();

    assert!(!glyph_font_ids.is_empty(), "no glyphs produced");
    for id in glyph_font_ids {
        let face = font_system.db().face(id).unwrap();
        assert!(
            face.monospaced,
            "expected a monospace face for `Family::Monospace` text, got {:?}",
            face.families
        );
    }
}

/// When the default monospace family resolves to an installed font,
/// shaping with `Family::Monospace` uses that font for every word.
#[test]
fn monospace_family_default_installed() {
    let mut font_system = font_system();
    font_system.db_mut().set_monospace_family("Fira Mono");

    let metrics = Metrics::new(14.0, 20.0);
    let mut buffer = Buffer::new(&mut font_system, metrics);

    // Fira Mono Medium registers at weight Medium, so request that weight
    // to also exercise the exact-weight default match.
    buffer.set_text(
        SPACED_TEXT,
        &Attrs::new()
            .family(Family::Monospace)
            .weight(cosmic_text::Weight::MEDIUM),
        Shaping::Advanced,
        None,
    );
    buffer.shape_until_scroll(&mut font_system, false);

    let glyph_font_ids: Vec<fontdb::ID> = buffer
        .layout_runs()
        .flat_map(|run| run.glyphs.iter().map(|g| g.font_id))
        .collect();

    assert!(!glyph_font_ids.is_empty(), "no glyphs produced");
    for id in glyph_font_ids {
        let face = font_system.db().face(id).unwrap();
        assert!(
            face.families.iter().any(|(name, _)| name == "Fira Mono"),
            "expected the default monospace family (Fira Mono), got {:?}",
            face.families
        );
    }
}

/// `Family::Monospace` and an explicit `Family::Name` of the same font
/// must pick the same face for the same text (the generic family must not
/// change which font is used, only how it is found).
#[test]
fn monospace_family_matches_explicit_name() {
    let mut font_system = font_system();
    let metrics = Metrics::new(14.0, 20.0);
    // Fira Mono Medium registers at weight Medium.
    let weight = cosmic_text::Weight::MEDIUM;

    let mut generic = Buffer::new(&mut font_system, metrics);
    generic.set_text(
        SPACED_TEXT,
        &Attrs::new().family(Family::Monospace).weight(weight),
        Shaping::Advanced,
        None,
    );
    generic.shape_until_scroll(&mut font_system, false);
    let generic_ids: Vec<fontdb::ID> = generic
        .layout_runs()
        .flat_map(|run| run.glyphs.iter().map(|g| g.font_id))
        .collect();

    let mut explicit = Buffer::new(&mut font_system, metrics);
    explicit.set_text(
        SPACED_TEXT,
        &Attrs::new()
            .family(Family::Name("Fira Mono"))
            .weight(weight),
        Shaping::Advanced,
        None,
    );
    explicit.shape_until_scroll(&mut font_system, false);
    let explicit_ids: Vec<fontdb::ID> = explicit
        .layout_runs()
        .flat_map(|run| run.glyphs.iter().map(|g| g.font_id))
        .collect();

    assert_eq!(
        generic_ids, explicit_ids,
        "`Family::Monospace` and `Family::Name(\"Fira Mono\")` picked different fonts"
    );
}
