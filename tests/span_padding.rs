//! Tests for [`SpanPadding`] integration with shape/layout calculations.
//!
//! Padding semantics implemented in `src/shape.rs`:
//! - `start`/`end` horizontal padding is attributed to words: a boundary at
//!   byte `b` contributes its `start` padding to the word whose content starts
//!   at `b` and its `end` padding to the word whose content ends at `b` (so
//!   each boundary is counted exactly once). The attributed padding is
//!   included in the word's width, so wrapping and ellipsization account for
//!   it.
//! - Horizontal padding is placed at the boundary's byte offset: a boundary
//!   inside a word is emitted between the two adjacent glyph clusters at that
//!   offset (sub-word precision), while boundaries at a word's edges are
//!   emitted on the word's x-stream edges. If a boundary falls strictly inside
//!   a single glyph cluster (e.g. a ligature) it is emitted after that
//!   cluster — sub-cluster precision is not tracked.
//! - Direction-aware placement: the *logical* start side of a span maps to the
//!   x-stream lead side when the span direction is congruent with the line
//!   direction (LTR span in LTR line, RTL span in RTL line), and to the
//!   x-stream trail side when incongruent (RTL span in LTR line, LTR span in
//!   RTL line).
//! - Across a wrap, each padding boundary is emitted on the visual line that
//!   contains the glyph after which it is placed.

use cosmic_text::{
    fontdb, Align, Attrs, Buffer, Direction, Ellipsize, FontSystem, Metrics, Shaping, SpanPadding,
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

struct Glyph {
    x: f32,
    w: f32,
    start: usize,
    end: usize,
}

struct Line {
    w: f32,
    max_ascent: f32,
    max_descent: f32,
    glyphs: Vec<Glyph>,
}

fn line_from(line: &cosmic_text::LayoutLine) -> Line {
    Line {
        w: line.w,
        max_ascent: line.max_ascent,
        max_descent: line.max_descent,
        glyphs: line
            .glyphs
            .iter()
            .map(|g| Glyph {
                x: g.x,
                w: g.w,
                start: g.start,
                end: g.end,
            })
            .collect(),
    }
}

/// Lays out `parts` (text with per-part `SpanPadding`) and returns the lines.
fn layout_parts(
    parts: &[(&str, SpanPadding)],
    wrap: Wrap,
    width: Option<f32>,
    direction: Direction,
) -> Vec<Line> {
    let mut font_system = font_system();
    let mut buffer = Buffer::new(&mut font_system, Metrics::new(FONT_SIZE, FONT_SIZE * 1.4));
    buffer.set_wrap(wrap);
    buffer.set_direction(direction);
    let defaults = Attrs::new();
    let spans: Vec<(&str, Attrs)> = parts
        .iter()
        .map(|(text, padding)| (*text, defaults.clone().padding(*padding)))
        .collect();
    buffer.set_rich_text(spans, &defaults, Shaping::Advanced, None);
    let mut buffer = buffer.borrow_with(&mut font_system);
    buffer.set_size(width, None);
    buffer
        .line_layout(0)
        .expect("expected at least one line")
        .iter()
        .map(line_from)
        .collect()
}

/// Lays out a single text with padding held in the default `Attrs`.
fn layout_defaults(text: &str, attrs: &Attrs, wrap: Wrap, width: Option<f32>) -> Vec<Line> {
    let mut font_system = font_system();
    let mut buffer = Buffer::new(&mut font_system, Metrics::new(FONT_SIZE, FONT_SIZE * 1.4));
    buffer.set_wrap(wrap);
    buffer.set_text(text, attrs, Shaping::Advanced, None);
    let mut buffer = buffer.borrow_with(&mut font_system);
    buffer.set_size(width, None);
    buffer
        .line_layout(0)
        .expect("expected at least one line")
        .iter()
        .map(line_from)
        .collect()
}

/// Lays out with center alignment.
fn layout_centered(parts: &[(&str, SpanPadding)], width: f32) -> Vec<Line> {
    let mut font_system = font_system();
    let mut buffer = Buffer::new(&mut font_system, Metrics::new(FONT_SIZE, FONT_SIZE * 1.4));
    buffer.set_wrap(Wrap::None);
    let defaults = Attrs::new();
    let spans: Vec<(&str, Attrs)> = parts
        .iter()
        .map(|(text, padding)| (*text, defaults.clone().padding(*padding)))
        .collect();
    buffer.set_rich_text(spans, &defaults, Shaping::Advanced, Some(Align::Center));
    let mut buffer = buffer.borrow_with(&mut font_system);
    buffer.set_size(Some(width), None);
    buffer
        .line_layout(0)
        .expect("expected at least one line")
        .iter()
        .map(line_from)
        .collect()
}

/// Lays out with trailing ellipsization.
fn layout_ellipsized(parts: &[(&str, SpanPadding)], width: f32) -> Vec<Line> {
    let mut font_system = font_system();
    let mut buffer = Buffer::new(&mut font_system, Metrics::new(FONT_SIZE, FONT_SIZE * 1.4));
    buffer.set_wrap(Wrap::None);
    buffer.set_ellipsize(Ellipsize::End(cosmic_text::EllipsizeHeightLimit::Lines(1)));
    let defaults = Attrs::new();
    let spans: Vec<(&str, Attrs)> = parts
        .iter()
        .map(|(text, padding)| (*text, defaults.clone().padding(*padding)))
        .collect();
    buffer.set_rich_text(spans, &defaults, Shaping::Advanced, None);
    let mut buffer = buffer.borrow_with(&mut font_system);
    buffer.set_size(Some(width), None);
    buffer
        .line_layout(0)
        .expect("expected at least one line")
        .iter()
        .map(line_from)
        .collect()
}

fn assert_close(a: f32, b: f32, msg: &str) {
    assert!(
        (a - b).abs() < EPS,
        "{msg}: expected {b}, got {a} (diff {})",
        (a - b).abs()
    );
}

/// Finds the baseline glyph for the same byte offset.
fn base_x(base: &Line, start: usize) -> f32 {
    base.glyphs
        .iter()
        .find(|g| g.start == start)
        .expect("baseline glyph")
        .x
}

#[test]
fn ltr_start_end_padding_widens_line() {
    let text = "hello";
    let base = layout_parts(
        &[(text, SpanPadding::ZERO)],
        Wrap::None,
        Some(500.0),
        Direction::Auto,
    );
    let pad = layout_parts(
        &[(text, SpanPadding::new(5.0, 7.0))],
        Wrap::None,
        Some(500.0),
        Direction::Auto,
    );
    assert_eq!(pad.len(), 1);
    // The line is widened by start + end padding.
    assert_close(pad[0].w, base[0].w + 12.0, "line width");
    // In an LTR line the start padding sits on the logical start (left) side.
    assert_close(pad[0].glyphs[0].x, 5.0, "first glyph x");
    // The end padding sits on the logical end (right) side.
    let last = pad[0].glyphs.last().unwrap();
    assert_close(last.x + last.w + 7.0, pad[0].w, "last glyph right edge");
    // Glyphs keep their relative spacing.
    for (b, p) in base[0].glyphs.iter().zip(pad[0].glyphs.iter()) {
        assert_close(p.w, b.w, "glyph width");
        assert_close(p.x - b.x, 5.0, "glyph shift");
    }
}

#[test]
fn rtl_start_end_padding_sides() {
    // "שלום" — auto-detected RTL line.
    let text = "שלום";
    let base = layout_parts(
        &[(text, SpanPadding::ZERO)],
        Wrap::None,
        Some(500.0),
        Direction::Auto,
    );
    let pad = layout_parts(
        &[(text, SpanPadding::new(5.0, 7.0))],
        Wrap::None,
        Some(500.0),
        Direction::Auto,
    );
    assert_eq!(pad.len(), 1);
    assert_close(pad[0].w, base[0].w + 12.0, "line width");
    // In an RTL line the start padding sits on the logical start (right)
    // side: the rightmost glyph's right edge is 5px short of the line's right
    // edge.
    let right_edge = pad[0]
        .glyphs
        .iter()
        .map(|g| g.x + g.w)
        .fold(0.0f32, f32::max);
    assert_close(right_edge, 500.0 - 5.0, "rightmost glyph right edge");
    // The end padding sits on the logical end (left) side.
    let left_edge = pad[0].glyphs.iter().map(|g| g.x).fold(f32::MAX, f32::min);
    assert_close(left_edge, 500.0 - pad[0].w + 7.0, "leftmost glyph x");
}

#[test]
fn padded_word_shifts_following_content() {
    // "aa bb cc" — pad the middle word: start 2, end 4.
    let base = layout_parts(
        &[("aa bb cc", SpanPadding::ZERO)],
        Wrap::None,
        Some(500.0),
        Direction::Auto,
    );
    let pad = layout_parts(
        &[
            ("aa ", SpanPadding::ZERO),
            ("bb", SpanPadding::new(2.0, 4.0)),
            (" cc", SpanPadding::ZERO),
        ],
        Wrap::None,
        Some(500.0),
        Direction::Auto,
    );
    assert_eq!(base.len(), 1);
    assert_eq!(pad.len(), 1);
    // Only the padded word's padding is added once (no double attribution at
    // the word boundaries).
    assert_close(pad[0].w, base[0].w + 6.0, "line width");
    for g in &pad[0].glyphs {
        // Bytes: a=0 a=1 ' '=2 b=3 b=4 ' '=5 c=6 c=7
        let shift = if g.start >= 3 && g.start < 5 {
            2.0 // inside the padded word: after its start padding
        } else if g.start >= 5 {
            6.0 // after the padded word: shifted by both paddings
        } else {
            0.0
        };
        assert_close(g.x, base_x(&base[0], g.start) + shift, "glyph shift");
    }
}

#[test]
fn padding_can_force_an_extra_wrap() {
    // "aa bb cc dd" — at width 40 the unpadded "bb cc" shares a line; padding
    // "bb" with 5+5 pushes "cc" to its own line.
    let base = layout_parts(
        &[("aa bb cc dd", SpanPadding::ZERO)],
        Wrap::Word,
        Some(40.0),
        Direction::Auto,
    );
    let pad = layout_parts(
        &[
            ("aa ", SpanPadding::ZERO),
            ("bb", SpanPadding::new(5.0, 5.0)),
            (" cc dd", SpanPadding::ZERO),
        ],
        Wrap::Word,
        Some(40.0),
        Direction::Auto,
    );
    assert_eq!(base.len(), 2, "baseline should be 2 lines");
    assert_eq!(pad.len(), 3, "padded should be 3 lines");
    // Baseline lines: "aa bb" / "cc dd"; the "bb" glyphs are the last two
    // glyphs of the first baseline line.
    let base_bb_w: f32 = base[0].glyphs.iter().rev().take(2).map(|g| g.w).sum();
    // The "bb" line carries both paddings.
    assert_close(pad[1].w, base_bb_w + 10.0, "bb line width");
    assert_close(pad[1].glyphs[0].x, 5.0, "bb first glyph x");
    // The "cc dd" line is unaffected by the padding.
    assert_close(
        pad[2].w,
        base[1].glyphs.iter().map(|g| g.w).sum::<f32>(),
        "cc dd line width",
    );
}

#[test]
fn word_wrap_keeps_padding_with_its_line() {
    // "world foo bar" with a span over "world foo": start padding 3 at the
    // span's logical start, end padding 4 at its logical end. At width 45 the
    // wrap falls *inside* the padded span: the start padding rides with the
    // "world" line and the end padding with the "foo" line.
    // Bytes: world=0..5 sp=5 foo=6..9 sp=9 bar=10..13.
    let base = layout_parts(
        &[("world foo bar", SpanPadding::ZERO)],
        Wrap::Word,
        Some(45.0),
        Direction::Auto,
    );
    let pad = layout_parts(
        &[
            ("world foo", SpanPadding::new(3.0, 4.0)),
            (" bar", SpanPadding::ZERO),
        ],
        Wrap::Word,
        Some(45.0),
        Direction::Auto,
    );
    assert_eq!(base.len(), 3);
    assert_eq!(pad.len(), 3);
    // Baseline lines at width 45: "world" / "foo" / "bar". The padded span
    // "world foo" is split by the wrap: the start padding (3) rides with the
    // "world" line and the end padding (4) with the "foo" line.
    let pad_deltas = [3.0, 4.0, 0.0];
    for (i, (b, p)) in base.iter().zip(pad.iter()).enumerate() {
        assert_close(p.w, b.w + pad_deltas[i], &format!("line {i} width"));
    }
    // The "world" line's start padding is on the left (LTR).
    assert_close(pad[0].glyphs[0].x, 3.0, "world first glyph x");
    // No padding leaks onto the non-span line.
    assert_close(
        pad[2].glyphs[0].x,
        base[2].glyphs[0].x,
        "line 2 first glyph x",
    );
}

#[test]
fn glyph_wrap_distributes_padding_over_lines() {
    // "hello" padded 5+5, glyph-wrapped at width 20. The x-stream-lead
    // padding lands on the line receiving the word's first glyph and the
    // x-stream-trail padding on the line receiving its last glyph. (At this
    // width the padding itself pushes "e" onto the next line, so the padded
    // layout has one more line than the baseline — the split points are
    // compared through the individual glyphs.)
    let base = layout_parts(
        &[("hello", SpanPadding::ZERO)],
        Wrap::Glyph,
        Some(20.0),
        Direction::Auto,
    );
    let pad = layout_parts(
        &[("hello", SpanPadding::new(5.0, 5.0))],
        Wrap::Glyph,
        Some(20.0),
        Direction::Auto,
    );
    assert_eq!(base.len(), 2, "baseline should be 2 lines");
    assert_eq!(pad.len(), 3, "padded should be 3 lines");
    // First line: just "h" plus the lead padding.
    assert_close(pad[0].w, base[0].glyphs[0].w + 5.0, "first line width");
    assert_eq!(pad[0].glyphs.len(), 1);
    assert_close(pad[0].glyphs[0].x, 5.0, "first glyph x");
    // Last line: just "o" plus the trail padding.
    assert_eq!(pad[2].glyphs.len(), 1);
    assert_close(
        pad[2].w,
        base[1].glyphs.last().unwrap().w + 5.0,
        "last line width",
    );
    // Total padding is counted exactly once.
    let total_pad_w: f32 = pad.iter().map(|l| l.w).sum();
    let total_base_w: f32 = base.iter().map(|l| l.w).sum();
    assert_close(total_pad_w, total_base_w + 10.0, "total width");
}

#[test]
fn word_or_glyph_wrap_includes_padding_in_word_width() {
    // "hello world" padded 2+3, WordOrGlyph at width 45: "hello" (+2) and
    // "world" (+3) each fit on their own line.
    let base = layout_parts(
        &[("hello world", SpanPadding::ZERO)],
        Wrap::WordOrGlyph,
        Some(45.0),
        Direction::Auto,
    );
    let pad = layout_parts(
        &[
            ("hello ", SpanPadding::new(2.0, 0.0)),
            ("world", SpanPadding::new(0.0, 3.0)),
        ],
        Wrap::WordOrGlyph,
        Some(45.0),
        Direction::Auto,
    );
    assert_eq!(base.len(), 2);
    assert_eq!(pad.len(), 2);
    assert_close(pad[0].w, base[0].w + 2.0, "hello line width");
    assert_close(pad[0].glyphs[0].x, 2.0, "hello first glyph x");
    assert_close(pad[1].w, base[1].w + 3.0, "world line width");
}

#[test]
fn rtl_line_with_ltr_run_padding() {
    // "بب aa جج" — forced RTL line; the LTR run "aa" (incongruent even span)
    // is padded with start 4 / end 6.
    // Bytes: ب=0..1 ب=2..3 sp=4 a=5 a=6 sp=7 ج=8..9 ج=10..11.
    let base = layout_parts(
        &[("بب aa جج", SpanPadding::ZERO)],
        Wrap::None,
        Some(300.0),
        Direction::RightToLeft,
    );
    let pad = layout_parts(
        &[
            ("بب ", SpanPadding::ZERO),
            ("aa", SpanPadding::new(4.0, 6.0)),
            (" جج", SpanPadding::ZERO),
        ],
        Wrap::None,
        Some(300.0),
        Direction::RightToLeft,
    );
    assert_eq!(base.len(), 1);
    assert_eq!(pad.len(), 1);
    assert_close(pad[0].w, base[0].w + 10.0, "line width");
    for g in &pad[0].glyphs {
        // In an RTL line the run's logical end faces the preceding (right)
        // content: the end padding (6) sits between the right neighbor and the
        // run, shifting the run 6 left. The logical start faces the trailing
        // (left) content: the start padding (4) shifts everything after the
        // run by 4 more.
        let shift = if g.start <= 4 {
            0.0
        } else if g.start < 7 {
            6.0
        } else {
            10.0
        };
        assert_close(g.x, base_x(&base[0], g.start) - shift, "glyph shift");
    }
    // The visible gaps: 6px between the right neighbor and the run, 4px
    // between the run and the left neighbor.
    let sp_right = pad[0].glyphs.iter().find(|g| g.start == 4).unwrap();
    let a_end = pad[0].glyphs.iter().find(|g| g.start == 6).unwrap();
    let a_start = pad[0].glyphs.iter().find(|g| g.start == 5).unwrap();
    let sp_left = pad[0].glyphs.iter().find(|g| g.start == 7).unwrap();
    assert_close(
        sp_right.x - (a_end.x + a_end.w),
        6.0,
        "gap between right neighbor and run",
    );
    assert_close(
        a_start.x - (sp_left.x + sp_left.w),
        4.0,
        "gap between run and left neighbor",
    );
}

#[test]
fn ltr_line_with_rtl_word_padding() {
    // "hi שלום bye" — forced LTR line; the RTL word (incongruent odd span) is
    // padded with start 4 / end 6.
    // Bytes: h=0 i=1 sp=2 שלום=3..11 sp=11 b=12 y=13 e=14.
    let base = layout_parts(
        &[("hi שלום bye", SpanPadding::ZERO)],
        Wrap::None,
        Some(300.0),
        Direction::LeftToRight,
    );
    let pad = layout_parts(
        &[
            ("hi ", SpanPadding::ZERO),
            ("שלום", SpanPadding::new(4.0, 6.0)),
            (" bye", SpanPadding::ZERO),
        ],
        Wrap::None,
        Some(300.0),
        Direction::LeftToRight,
    );
    assert_eq!(base.len(), 1);
    assert_eq!(pad.len(), 1);
    assert_close(pad[0].w, base[0].w + 10.0, "line width");
    for g in &pad[0].glyphs {
        // The word's logical end faces the word's visual left side in an LTR
        // line: the end padding (6) shifts the whole word right by 6. The
        // logical start faces the visual right side: the start padding (4)
        // shifts everything after the word by 4 more.
        let shift = if g.start < 3 {
            0.0
        } else if g.start < 11 {
            6.0
        } else {
            10.0
        };
        assert_close(g.x, base_x(&base[0], g.start) + shift, "glyph shift");
    }
    // The visible gaps: 6px between "hi" and the word's left edge (its
    // logical end), 4px between the word's right edge (its logical start)
    // and "bye". For the RTL word "שלום" the leftmost glyph is the last
    // logical char (start byte 9) and the rightmost is the first (start 3).
    let sp_before = pad[0].glyphs.iter().find(|g| g.start == 2).unwrap();
    let w_left = pad[0].glyphs.iter().find(|g| g.start == 9).unwrap();
    let w_right = pad[0].glyphs.iter().find(|g| g.start == 3).unwrap();
    let sp_after = pad[0].glyphs.iter().find(|g| g.start == 11).unwrap();
    assert_close(
        w_left.x - (sp_before.x + sp_before.w),
        6.0,
        "gap before word",
    );
    assert_close(sp_after.x - (w_right.x + w_right.w), 4.0, "gap after word");
}

#[test]
fn rtl_word_wrap_padding_sides() {
    // "שלום עולם" — auto-detected RTL line, word-wrapped at width 60 so each
    // word gets its own line; the span (whole line) is padded 5+5, so the
    // start padding lands on the first line's right edge and the end padding
    // on the last line's left edge.
    let text = "שלום עולם";
    let base = layout_parts(
        &[(text, SpanPadding::ZERO)],
        Wrap::Word,
        Some(60.0),
        Direction::Auto,
    );
    let pad = layout_parts(
        &[(text, SpanPadding::new(5.0, 5.0))],
        Wrap::Word,
        Some(60.0),
        Direction::Auto,
    );
    assert_eq!(base.len(), 2);
    assert_eq!(pad.len(), 2);
    assert_close(pad[0].w, base[0].w + 5.0, "first line width");
    assert_close(pad[1].w, base[1].w + 5.0, "last line width");
    // First line: start padding on the right side.
    let right_edge = pad[0]
        .glyphs
        .iter()
        .map(|g| g.x + g.w)
        .fold(0.0f32, f32::max);
    assert_close(right_edge, 60.0 - 5.0, "first line right edge");
    // Last line: end padding on the left side.
    let left_edge = pad[1].glyphs.iter().map(|g| g.x).fold(f32::MAX, f32::min);
    assert_close(left_edge, 60.0 - pad[1].w + 5.0, "last line left edge");
}

#[test]
fn defaults_padding_applies_to_whole_line() {
    // Padding held in the default `Attrs` (not in an explicit span).
    let text = "hello";
    let base = layout_defaults(text, &Attrs::new(), Wrap::None, Some(500.0));
    let pad = layout_defaults(
        text,
        &Attrs::new().padding(SpanPadding::new(5.0, 5.0)),
        Wrap::None,
        Some(500.0),
    );
    assert_eq!(pad.len(), 1);
    assert_close(pad[0].w, base[0].w + 10.0, "line width");
    assert_close(pad[0].glyphs[0].x, 5.0, "first glyph x");
    let last = pad[0].glyphs.last().unwrap();
    assert_close(last.x + last.w + 5.0, pad[0].w, "last glyph right edge");
}

#[test]
fn zero_padding_is_a_noop() {
    let base = layout_parts(
        &[("hello world", SpanPadding::ZERO)],
        Wrap::Word,
        Some(50.0),
        Direction::Auto,
    );
    let pad = layout_parts(
        &[
            ("hello ", SpanPadding::new(0.0, 0.0)),
            ("world", SpanPadding::new(0.0, 0.0)),
        ],
        Wrap::Word,
        Some(50.0),
        Direction::Auto,
    );
    assert_eq!(base.len(), pad.len());
    for (b, p) in base.iter().zip(pad.iter()) {
        assert_close(p.w, b.w, "line width");
        assert_close(p.max_ascent, b.max_ascent, "max ascent");
        assert_close(p.max_descent, b.max_descent, "max descent");
        assert_eq!(p.glyphs.len(), b.glyphs.len());
        for (bg, pg) in b.glyphs.iter().zip(p.glyphs.iter()) {
            assert_close(pg.x, bg.x, "glyph x");
            assert_close(pg.w, bg.w, "glyph w");
        }
    }
}

#[test]
fn centered_alignment_includes_padding() {
    // "hello" padded 5+7, centered in 100px: the centering uses the padded
    // line width, and the paddings sit at the padded line's edges.
    let pad = layout_centered(&[("hello", SpanPadding::new(5.0, 7.0))], 100.0);
    assert_eq!(pad.len(), 1);
    let line = &pad[0];
    let inset = (100.0 - line.w) / 2.0;
    assert_close(line.glyphs[0].x, inset + 5.0, "first glyph x");
    let last = line.glyphs.last().unwrap();
    assert_close(
        last.x + last.w,
        100.0 - inset - 7.0,
        "last glyph right edge",
    );
}

#[test]
fn ellipsize_with_padding_smoke() {
    // Documented approximation: the ellipsize fit calculation ignores the
    // overflowing word's padding; the smoke test only checks that layout
    // produces a sane ellipsized line (one line, ellipsis glyph present,
    // within the width).
    let lines = layout_ellipsized(
        &[
            ("hello ", SpanPadding::ZERO),
            ("world", SpanPadding::new(3.0, 3.0)),
        ],
        40.0,
    );
    assert_eq!(lines.len(), 1);
    let line = &lines[0];
    assert!(line.w <= 40.0 + EPS, "line width {} > 40", line.w);
    // The ellipsis glyph's byte range is collapsed to the elision boundary.
    assert!(
        line.glyphs.iter().any(|g| g.start == g.end),
        "expected an ellipsis glyph"
    );
}

#[test]
fn mid_word_padding_is_placed_between_glyphs() {
    // "hello" with a span over just "he": start padding 2, end padding 3.
    // The span's end falls *inside* the word "hello" (at byte 2, between "e"
    // and "l"), so the end padding must be emitted between those two glyphs
    // rather than at the word's edge.
    let base = layout_parts(
        &[("hello", SpanPadding::ZERO)],
        Wrap::None,
        Some(500.0),
        Direction::Auto,
    );
    let pad = layout_parts(
        &[
            ("he", SpanPadding::new(2.0, 3.0)),
            ("llo", SpanPadding::ZERO),
        ],
        Wrap::None,
        Some(500.0),
        Direction::Auto,
    );
    assert_eq!(base.len(), 1);
    assert_eq!(pad.len(), 1);
    // The line is widened by the full start + end padding.
    assert_close(pad[0].w, base[0].w + 5.0, "line width");
    // The start padding sits before the first glyph (LTR).
    assert_close(pad[0].glyphs[0].x, 2.0, "first glyph x");
    // "h" and "e" are shifted by the start padding only; "l", "l", "o" are
    // shifted by start + end padding.
    for g in &pad[0].glyphs {
        let shift = if g.start < 2 { 2.0 } else { 5.0 };
        assert_close(g.x, base_x(&base[0], g.start) + shift, "glyph shift");
    }
    // The 3px end padding is the visible gap between "e" (byte 1) and
    // "l" (byte 2).
    let e = pad[0].glyphs.iter().find(|g| g.start == 1).unwrap();
    let l = pad[0].glyphs.iter().find(|g| g.start == 2).unwrap();
    assert_close(l.x - (e.x + e.w), 3.0, "gap between 'e' and 'l'");
}

#[test]
fn mid_word_padding_incongruent_rtl_word() {
    // "hi שלום bye" — forced LTR line; a span over just "של" (bytes 3..7)
    // within the RTL word "שלום". The span's end (byte 7) falls inside the
    // word, between "ל" and "ו", and its start (byte 3) is at the word's
    // logical start ("ש", the rightmost glyph in an LTR line).
    let base = layout_parts(
        &[("hi שלום bye", SpanPadding::ZERO)],
        Wrap::None,
        Some(500.0),
        Direction::LeftToRight,
    );
    let pad = layout_parts(
        &[
            ("hi ", SpanPadding::ZERO),
            ("של", SpanPadding::new(4.0, 3.0)),
            ("ום bye", SpanPadding::ZERO),
        ],
        Wrap::None,
        Some(500.0),
        Direction::LeftToRight,
    );
    assert_eq!(base.len(), 1);
    assert_eq!(pad.len(), 1);
    assert_close(pad[0].w, base[0].w + 7.0, "line width");
    // Bytes: h=0 i=1 sp=2 ש=3..5 ל=5..7 ו=7..9 ם=9..11 sp=11 b=12 y=13 e=14.
    // The 3px end padding (byte 7) sits between "ו" and "ל", pushing "ל"/"ש"
    // (and everything after the word) right by 3. The 4px start padding sits
    // to the right of "ש" (the word's logical start), pushing the trailing
    // content right by a further 4.
    for g in &pad[0].glyphs {
        let shift = match g.start {
            0..=2 => 0.0, // "hi "
            9 => 0.0,     // "ם" (leftmost glyph)
            7 => 0.0,     // "ו"
            3 | 5 => 3.0, // "ל" / "ש"
            _ => 7.0,     // trailing " bye"
        };
        assert_close(g.x, base_x(&base[0], g.start) + shift, "glyph shift");
    }
    // Visible gaps: 3px between "ו" (right edge) and "ל", and 4px between
    // "ש" (right edge) and the trailing space.
    let vav = pad[0].glyphs.iter().find(|g| g.start == 7).unwrap();
    let lamed = pad[0].glyphs.iter().find(|g| g.start == 5).unwrap();
    let shin = pad[0].glyphs.iter().find(|g| g.start == 3).unwrap();
    let sp = pad[0].glyphs.iter().find(|g| g.start == 11).unwrap();
    assert_close(lamed.x - (vav.x + vav.w), 3.0, "gap between 'ו' and 'ל'");
    assert_close(sp.x - (shin.x + shin.w), 4.0, "gap after the word");
}
