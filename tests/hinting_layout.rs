//! Layout measurements must stay consistent with the laid out glyph positions
//! when [`cosmic_text::Hinting::Enabled`] is used: glyph advances are rounded
//! to whole pixels during layout, so line widths and alignment corrections
//! computed from the unhinted advances would no longer match.
use cosmic_text::{
    fontdb, Align, Attrs, AttrsList, Direction, Ellipsize, EllipsizeHeightLimit, Family,
    FontSystem, Hinting, LayoutLine, Metrics, ShapeLine, Shaping, Weight, Wrap,
};

const EPSILON: f32 = 0.01;

/// Content span of the laid out glyphs: rightmost edge minus leftmost edge.
fn content_span(line: &LayoutLine) -> f32 {
    let max_edge = line.glyphs.iter().map(|g| g.x + g.w).fold(0.0, f32::max);
    let min_x = line.glyphs.iter().map(|g| g.x).fold(f32::MAX, f32::min);
    max_edge - min_x
}

fn min_x(line: &LayoutLine) -> f32 {
    line.glyphs.iter().map(|g| g.x).fold(f32::MAX, f32::min)
}

fn max_edge(line: &LayoutLine) -> f32 {
    line.glyphs.iter().map(|g| g.x + g.w).fold(0.0, f32::max)
}

fn test_font_system() -> FontSystem {
    let mut font_system =
        FontSystem::new_with_locale_and_db("en-US".into(), fontdb::Database::new());
    let font = std::fs::read("fonts/FiraMono-Medium.ttf").unwrap();
    font_system.db_mut().load_font_data(font);
    font_system
}

fn attrs() -> AttrsList {
    AttrsList::new(
        &Attrs::new()
            .family(Family::Name("FiraMono"))
            .weight(Weight::MEDIUM),
    )
}

/// The reported line width must match the span the glyphs actually occupy for
/// every alignment, with and without hinting.
#[test]
fn line_width_matches_glyph_span() {
    let mut font_system = test_font_system();
    let attrs = attrs();
    let font_size = 18.0;

    let line = ShapeLine::new(
        &mut font_system,
        "hello world\nfoobar\na long line to wrap the line\nend",
        &attrs,
        Shaping::Advanced,
        8,
        Direction::Auto,
    );

    for hinting in [Hinting::Disabled, Hinting::Enabled] {
        for align in [
            Align::Left,
            Align::Right,
            Align::Center,
            Align::End,
            Align::Justified,
        ] {
            let layout = line.layout(
                font_size,
                Some(200.0),
                Wrap::Word,
                Some(align),
                None,
                hinting,
            );
            for layout_line in layout.iter().filter(|l| !l.glyphs.is_empty()) {
                assert!(
                    (layout_line.w - content_span(layout_line)).abs() < EPSILON,
                    "hinting {hinting:?} align {align:?}: w {:.4} != span {:.4}",
                    layout_line.w,
                    content_span(layout_line)
                );
            }
        }
    }
}

/// Aligned lines must line up at their actual (hinted) edges: right/End
/// aligned fitting lines end exactly at the right edge, centered lines are
/// centered on half the width, and left aligned lines start at the origin.
#[test]
fn alignment_uses_hinted_widths() {
    let mut font_system = test_font_system();
    let attrs = attrs();
    let font_size = 18.0;
    let width = 200.0;

    let line = ShapeLine::new(
        &mut font_system,
        "hello world\nfoobar\na long line to wrap the line\nend",
        &attrs,
        Shaping::Advanced,
        8,
        Direction::Auto,
    );

    for hinting in [Hinting::Disabled, Hinting::Enabled] {
        for align in [Align::Left, Align::Right, Align::Center, Align::End] {
            let layout = line.layout(
                font_size,
                Some(width),
                Wrap::Word,
                Some(align),
                None,
                hinting,
            );
            for layout_line in layout.iter().filter(|l| !l.glyphs.is_empty()) {
                let fits = layout_line.w <= width;
                match align {
                    Align::Left => {
                        assert!(
                            min_x(layout_line).abs() < EPSILON,
                            "hinting {hinting:?}: left aligned line starts at {:.4}",
                            min_x(layout_line)
                        );
                    }
                    Align::Right | Align::End => {
                        if fits {
                            assert!(
                                (max_edge(layout_line) - width).abs() < EPSILON,
                                "hinting {hinting:?} align {align:?}: right edge at {:.4}, expected {width}",
                                max_edge(layout_line)
                            );
                        }
                    }
                    Align::Center => {
                        if fits {
                            let center = (min_x(layout_line) + max_edge(layout_line)) / 2.0;
                            assert!(
                                (center - width / 2.0).abs() <= 0.5 + EPSILON,
                                "hinting {hinting:?}: center at {center:.4}, expected {}",
                                width / 2.0
                            );
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Same invariants for right-to-left text.
#[test]
fn rtl_alignment_uses_hinted_widths() {
    let mut font_system = test_font_system();
    let attrs = attrs();
    let font_size = 18.0;
    // "היום מחר" ("today tomorrow")
    let rtl_text = "\u{5E2}\u{5D5}\u{5DD} \u{5DE}\u{5D7}\u{5E8}";

    let line = ShapeLine::new(
        &mut font_system,
        rtl_text,
        &attrs,
        Shaping::Advanced,
        8,
        Direction::RightToLeft,
    );

    for hinting in [Hinting::Disabled, Hinting::Enabled] {
        for width_opt in [None, Some(200.0), Some(80.0)] {
            let layout = line.layout(
                font_size,
                width_opt,
                Wrap::Word,
                Some(Align::Right),
                None,
                hinting,
            );
            for layout_line in layout.iter().filter(|l| !l.glyphs.is_empty()) {
                assert!(
                    (layout_line.w - content_span(layout_line)).abs() < EPSILON,
                    "hinting {hinting:?} width {width_opt:?}: w {:.4} != span {:.4}",
                    layout_line.w,
                    content_span(layout_line)
                );
                if let Some(width) = width_opt {
                    if layout_line.w <= width {
                        assert!(
                            (max_edge(layout_line) - width).abs() < EPSILON,
                            "hinting {hinting:?}: right edge at {:.4}, expected {width}",
                            max_edge(layout_line)
                        );
                    }
                }
            }
        }
    }
}

/// Unbounded layout with hinting: a single visual line starts at the origin
/// and its reported width matches the glyph span.
#[test]
fn unbounded_width_with_hinting() {
    let mut font_system = test_font_system();
    let attrs = attrs();
    let font_size = 18.0;

    let line = ShapeLine::new(
        &mut font_system,
        "hello world foobar",
        &attrs,
        Shaping::Advanced,
        8,
        Direction::Auto,
    );

    for hinting in [Hinting::Disabled, Hinting::Enabled] {
        let layout = line.layout(
            font_size,
            None,
            Wrap::Word,
            Some(Align::Right),
            None,
            hinting,
        );
        for layout_line in layout.iter().filter(|l| !l.glyphs.is_empty()) {
            assert!(min_x(layout_line).abs() < EPSILON);
            assert!(
                (layout_line.w - content_span(layout_line)).abs() < EPSILON,
                "hinting {hinting:?}: w {:.4} != span {:.4}",
                layout_line.w,
                content_span(layout_line)
            );
        }
    }
}

/// Ellipsized lines include the ellipsis glyphs in their reported width.
#[test]
fn ellipsized_line_width_matches_glyph_span() {
    let mut font_system = test_font_system();

    for hinting in [Hinting::Disabled, Hinting::Enabled] {
        let mut buffer = cosmic_text::Buffer::new(&mut font_system, Metrics::new(18.0, 24.0));
        buffer.set_wrap(Wrap::Word);
        buffer.set_size(Some(80.0), None);
        buffer.set_ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1)));
        buffer.set_hinting(hinting);
        buffer.set_text(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            &Attrs::new().family(Family::Name("FiraMono")),
            Shaping::Advanced,
            None,
        );
        buffer.shape_until_scroll(&mut font_system, false);
        for run in buffer.layout_runs() {
            if run.glyphs.is_empty() {
                continue;
            }
            let max_edge = run.glyphs.iter().map(|g| g.x + g.w).fold(0.0, f32::max);
            let min_x = run.glyphs.iter().map(|g| g.x).fold(f32::MAX, f32::min);
            assert!(
                (run.line_w - (max_edge - min_x)).abs() < EPSILON,
                "hinting {hinting:?}: line_w {:.4} != span {:.4}",
                run.line_w,
                max_edge - min_x
            );
        }
    }
}
