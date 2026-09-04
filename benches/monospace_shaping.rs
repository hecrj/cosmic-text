//! Benchmarks for <https://github.com/pop-os/cosmic-text/issues/518>:
//! shaping with the generic `Family::Monospace` used to rescan every face
//! in the database (and re-rank the monospace fallback candidates) for
//! every word - including every whitespace-only word - making it 10-20x
//! slower than shaping with an explicit `Family::Name`.
//!
//! Both benches shape whitespace-heavy text, where every whitespace
//! character is its own word and thus used to trigger a full monospace
//! fallback scan. With the fix, both should be fast and of the same order
//! of magnitude.

use cosmic_text as ct;
use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;
use std::path::PathBuf;

/// A `FontSystem` with a realistic number of faces but no "Noto Sans Mono"
/// (cosmic-text's default monospace family), so shaping with
/// `Family::Monospace` exercises the monospace fallback path for every word
/// against the whole database, like on a real system.
fn font_system() -> ct::FontSystem {
    let repo_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let fonts_path = PathBuf::from(&repo_dir).join("fonts");

    let fira_mono = std::fs::read(fonts_path.join("FiraMono-Medium.ttf")).unwrap();
    let inter = std::fs::read(fonts_path.join("Inter-Regular.ttf")).unwrap();

    let mut db = ct::fontdb::Database::new();
    // Load each font 50 times so the database has a realistic number of
    // faces to scan for every word.
    for _ in 0..50 {
        db.load_font_data(fira_mono.clone());
        db.load_font_data(inter.clone());
    }
    // Mirror the defaults set by `FontSystem` for the generic families.
    db.set_monospace_family("Noto Sans Mono");
    db.set_sans_serif_family("Open Sans");
    db.set_serif_family("DejaVu Serif");

    ct::FontSystem::new_with_locale_and_db("en-US".to_string(), db)
}

fn bench_issue_518(c: &mut Criterion) {
    let mut fs = font_system();
    let metrics = ct::Metrics::new(14.0, 20.0);

    // Whitespace-heavy text: every whitespace character is shaped as its
    // own word, so each one used to trigger a full monospace fallback scan.
    let text = "Label                    value\n".repeat(100);
    let mono_attrs = ct::Attrs::new()
        .family(ct::Family::Monospace)
        .weight(ct::Weight::MEDIUM);
    let name_attrs = ct::Attrs::new()
        .family(ct::Family::Name("Fira Mono"))
        .weight(ct::Weight::MEDIUM);

    let mut generic = ct::Buffer::new(&mut fs, metrics);
    generic.set_size(Some(500.0), None);
    let mut explicit = ct::Buffer::new(&mut fs, metrics);
    explicit.set_size(Some(500.0), None);

    let mut group = c.benchmark_group("issue_518");

    group.bench_function("Family::Monospace", |b| {
        b.iter(|| {
            generic.set_text(black_box(&text), &mono_attrs, ct::Shaping::Advanced, None);
            generic.shape_until_scroll(&mut fs, false);
        });
    });

    group.bench_function("Family::Name", |b| {
        b.iter(|| {
            explicit.set_text(black_box(&text), &name_attrs, ct::Shaping::Advanced, None);
            explicit.shape_until_scroll(&mut fs, false);
        });
    });

    group.finish();
}

criterion_group!(benches, bench_issue_518);
criterion_main!(benches);
