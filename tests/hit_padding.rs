//! Tests for [`Buffer::hit`] when the click falls within a span's
//! [`SpanPadding`] — a horizontal region between glyph boxes that has no
//! glyphs of its own.
//!
//! Padding is emitted as a plain x-gap during layout, so a click inside it is
//! not contained by any glyph box. The cursor must land at the padding
//! boundary (the nearest glyph edge), not at the end of the line.

use cosmic_text::{
    fontdb, Affinity, Attrs, Buffer, Cursor, Direction, FontSystem, Metrics, Shaping, SpanPadding,
    Wrap,
};

const FONT_SIZE: f32 = 14.0;
const EPS: f32 = 1e-3;

fn font_system() -> FontSystem {
    let mut font_system =
        FontSystem::new_with_locale_and_db("en-US".into(), fontdb::Database::new());
    font_system
        .db_mut()
        .load_font_data(std::fs::read("fonts/Inter-Regular.ttf").unwrap());
    font_system
        .db_mut()
        .load_font_data(std::fs::read("fonts/NotoSansArabic.ttf").unwrap());
    font_system
}

/// Lays out `parts` (text with per-part `SpanPadding`) into a shaped
/// single-line [`Buffer`].
fn make_buffer(parts: &[(&str, SpanPadding)], direction: Direction) -> (FontSystem, Buffer) {
    let mut font_system = font_system();
    let mut buffer = Buffer::new(&mut font_system, Metrics::new(FONT_SIZE, FONT_SIZE * 1.4));
    buffer.set_wrap(Wrap::None);
    buffer.set_direction(direction);
    let defaults = Attrs::new();
    let spans: Vec<(&str, Attrs)> = parts
        .iter()
        .map(|(text, padding)| (*text, defaults.clone().padding(*padding)))
        .collect();
    buffer.set_rich_text(spans, &defaults, Shaping::Advanced, None);
    buffer.set_size(Some(500.0), None);
    buffer.shape_until_scroll(&mut font_system, true);
    (font_system, buffer)
}

/// The `(gap_left, gap_right)` x-range of the glyph-free gap flanked by the
/// two x-adjacent glyphs starting at bytes `a_start` and `b_start`.
fn gap_between(buffer: &Buffer, a_start: usize, b_start: usize) -> (f32, f32) {
    let run = buffer.layout_runs().next().expect("expected a layout run");
    let a = run
        .glyphs
        .iter()
        .find(|g| g.start == a_start)
        .unwrap_or_else(|| panic!("expected a glyph starting at byte {a_start}"));
    let b = run
        .glyphs
        .iter()
        .find(|g| g.start == b_start)
        .unwrap_or_else(|| panic!("expected a glyph starting at byte {b_start}"));
    // The gap is bounded by the inner edges of the two boxes: pick the pair
    // that forms a (positive-width) gap.
    let (l1, r1) = (a.x + a.w, b.x);
    let (l2, r2) = (b.x + b.w, a.x);
    if r1 > l1 {
        (l1, r1)
    } else {
        assert!(
            r2 > l2,
            "glyphs at {a_start} and {b_start} are not adjacent"
        );
        (l2, r2)
    }
}

#[test]
fn hit_in_padding_gap_ltr() {
    // "hello " + "world" with 10px start padding on the second span: a 10px
    // glyph-free gap between the space (byte 5..6) and "w" (byte 6..7).
    let (_fs, buffer) = make_buffer(
        &[
            ("hello ", SpanPadding::ZERO),
            ("world", SpanPadding::new(10.0, 0.0)),
        ],
        Direction::Auto,
    );
    let (gap_left, gap_right) = gap_between(&buffer, 5, 6);
    assert!(
        gap_right - gap_left > 9.0,
        "expected a ~10px gap, got {}",
        gap_right - gap_left
    );

    // Clicking anywhere in the gap lands at the word boundary (byte 6), not
    // at the end of the line (byte 11). The side of the gap determines which
    // run the cursor is associated with.
    let cursor = buffer
        .hit(gap_left + (gap_right - gap_left) * 0.25, 5.0)
        .unwrap();
    assert_eq!(cursor, Cursor::new_with_affinity(0, 6, Affinity::Before));
    let cursor = buffer
        .hit(gap_left + (gap_right - gap_left) * 0.75, 5.0)
        .unwrap();
    assert_eq!(cursor, Cursor::new_with_affinity(0, 6, Affinity::After));

    // The cursor renders on the gap boundary (one of its edges), not at the
    // end of the line.
    for affinity in [Affinity::Before, Affinity::After] {
        let (x, _) = buffer
            .cursor_position(&Cursor::new_with_affinity(0, 6, affinity))
            .unwrap();
        assert!(
            x >= gap_left - EPS && x <= gap_right + EPS,
            "cursor x {x} should render at the gap boundary [{gap_left}, {gap_right}]"
        );
    }
}

#[test]
fn hit_in_mid_word_padding_gap_ltr() {
    // "he" (padded 2+3) + "llo": the span boundary falls inside the word,
    // leaving a 3px glyph-free gap between "e" (byte 1..2) and "l" (2..3).
    let (_fs, buffer) = make_buffer(
        &[
            ("he", SpanPadding::new(2.0, 3.0)),
            ("llo", SpanPadding::ZERO),
        ],
        Direction::Auto,
    );
    let (gap_left, gap_right) = gap_between(&buffer, 1, 2);
    assert!(
        gap_right - gap_left > 2.0,
        "expected a ~3px gap, got {}",
        gap_right - gap_left
    );

    let cursor = buffer
        .hit(gap_left + (gap_right - gap_left) * 0.25, 5.0)
        .unwrap();
    assert_eq!(cursor, Cursor::new_with_affinity(0, 2, Affinity::Before));
    let cursor = buffer
        .hit(gap_left + (gap_right - gap_left) * 0.75, 5.0)
        .unwrap();
    assert_eq!(cursor, Cursor::new_with_affinity(0, 2, Affinity::After));
}

#[test]
fn hit_in_trailing_padding_ltr() {
    // Padding after the last glyph: the gap is *past* all glyphs, so the
    // cursor goes to the end of the line (existing behavior).
    let (_fs, buffer) = make_buffer(
        &[("hello world", SpanPadding::new(0.0, 10.0))],
        Direction::Auto,
    );
    let run = buffer.layout_runs().next().unwrap();
    let last = run.glyphs.last().unwrap();
    let x = last.x + last.w + 5.0; // inside the trailing padding
    let cursor = buffer.hit(x, 5.0).unwrap();
    assert_eq!(cursor, Cursor::new_with_affinity(0, 11, Affinity::Before));
}

#[test]
fn hit_in_leading_padding_ltr() {
    // Padding before the first glyph: the cursor goes to the start of the
    // line (existing behavior).
    let (_fs, buffer) = make_buffer(
        &[("hello world", SpanPadding::new(10.0, 0.0))],
        Direction::Auto,
    );
    let run = buffer.layout_runs().next().unwrap();
    let first = run.glyphs.first().unwrap();
    let x = first.x / 2.0; // inside the leading padding
    let cursor = buffer.hit(x, 5.0).unwrap();
    assert_eq!(cursor, Cursor::new_with_affinity(0, 0, Affinity::After));
}

#[test]
fn hit_in_padding_gap_rtl() {
    // "שלום " + "עולם" with 10px start padding on the second span. In an RTL
    // line the span's logical start faces its x-stream lead (right) side, so
    // the gap sits between the space (byte 8..9) and "ע" (byte 9..11).
    // Bytes: ש=0..2 ל=2..4 ו=4..6 ם=6..8 sp=8..9 ע=9..11 ל=11..13 ו=13..15 ם=15..17.
    let (_fs, buffer) = make_buffer(
        &[
            ("שלום ", SpanPadding::ZERO),
            ("עולם", SpanPadding::new(10.0, 0.0)),
        ],
        Direction::Auto,
    );
    let (gap_left, gap_right) = gap_between(&buffer, 8, 9);
    assert!(
        gap_right - gap_left > 9.0,
        "expected a ~10px gap, got {}",
        gap_right - gap_left
    );

    // Clicking anywhere in the gap lands at the word boundary (byte 9).
    let cursor = buffer
        .hit(gap_left + (gap_right - gap_left) * 0.25, 5.0)
        .unwrap();
    assert_eq!((cursor.line, cursor.index), (0, 9));
    let cursor = buffer
        .hit(gap_left + (gap_right - gap_left) * 0.75, 5.0)
        .unwrap();
    assert_eq!((cursor.line, cursor.index), (0, 9));
}

#[test]
fn hit_in_padding_gap_ltr_line_with_rtl_word() {
    // "hi שלום bye" — LTR line, the RTL word "שלום" (bytes 3..11) padded 4+6.
    // The word's logical end faces its visual left side: a 6px gap between
    // the leading space (byte 2..3) and "ם" (byte 9..11). Its logical start
    // faces the visual right side: a 4px gap between "ש" (byte 3..5) and the
    // trailing space (byte 11..12).
    let (_fs, buffer) = make_buffer(
        &[
            ("hi ", SpanPadding::ZERO),
            ("שלום", SpanPadding::new(4.0, 6.0)),
            (" bye", SpanPadding::ZERO),
        ],
        Direction::LeftToRight,
    );

    // Left gap (between the leading space and "ם"): the nearest edges map to
    // byte 3 (after the space, LTR) on its left side and byte 11 (the
    // logical end of the RTL word) on its right side.
    let (gap_left, gap_right) = gap_between(&buffer, 2, 9);
    assert!(
        gap_right - gap_left > 5.0,
        "expected a ~6px gap, got {}",
        gap_right - gap_left
    );
    let cursor = buffer
        .hit(gap_left + (gap_right - gap_left) * 0.25, 5.0)
        .unwrap();
    assert_eq!(cursor, Cursor::new_with_affinity(0, 3, Affinity::Before));
    let cursor = buffer
        .hit(gap_left + (gap_right - gap_left) * 0.75, 5.0)
        .unwrap();
    assert_eq!(cursor, Cursor::new_with_affinity(0, 11, Affinity::Before));

    // Right gap (between "ש" and the trailing space): the nearest edges map
    // to byte 3 (the logical start of the RTL word) on its left side and
    // byte 11 (after the word) on its right side.
    let (gap_left, gap_right) = gap_between(&buffer, 3, 11);
    assert!(
        gap_right - gap_left > 3.0,
        "expected a ~4px gap, got {}",
        gap_right - gap_left
    );
    let cursor = buffer
        .hit(gap_left + (gap_right - gap_left) * 0.25, 5.0)
        .unwrap();
    assert_eq!(cursor, Cursor::new_with_affinity(0, 3, Affinity::After));
    let cursor = buffer
        .hit(gap_left + (gap_right - gap_left) * 0.75, 5.0)
        .unwrap();
    assert_eq!(cursor, Cursor::new_with_affinity(0, 11, Affinity::After));
}

#[test]
fn hit_without_padding_is_unchanged() {
    // Regression: on a line without padding, glyph boxes tile the line, so
    // the new gap branch is never reached and hit behaves as before.
    let (_fs, buffer) = make_buffer(&[("hello world", SpanPadding::ZERO)], Direction::Auto);
    let run = buffer.layout_runs().next().unwrap();

    // On the first glyph (left half: before the character).
    let h = run.glyphs.iter().find(|g| g.start == 0).unwrap();
    let cursor = buffer.hit(h.x + h.w / 4.0, 5.0).unwrap();
    assert_eq!((cursor.line, cursor.index), (0, 0));

    // Before the first glyph.
    let cursor = buffer.hit(h.x / 2.0, 5.0).unwrap();
    assert_eq!(cursor, Cursor::new_with_affinity(0, 0, Affinity::After));

    // Inside the space glyph (byte 5..6): left half before the space, right
    // half after it.
    let sp = run.glyphs.iter().find(|g| g.start == 5).unwrap();
    let cursor = buffer.hit(sp.x + sp.w / 4.0, 5.0).unwrap();
    assert_eq!((cursor.line, cursor.index), (0, 5));
    let cursor = buffer.hit(sp.x + 3.0 * sp.w / 4.0, 5.0).unwrap();
    assert_eq!((cursor.line, cursor.index), (0, 6));

    // Past the last glyph.
    let last = run.glyphs.last().unwrap();
    let cursor = buffer.hit(last.x + last.w + 50.0, 5.0).unwrap();
    assert_eq!(cursor, Cursor::new_with_affinity(0, 11, Affinity::Before));
}
