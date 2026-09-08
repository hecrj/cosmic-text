// SPDX-License-Identifier: MIT OR Apache-2.0

use core::fmt::Display;

use core::ops::Range;

use crate::{math, CacheKey, CacheKeyFlags, Color, GlyphDecorationData};
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

#[cfg(not(feature = "std"))]
use core_maths::CoreFloat;

/// A laid out glyph
#[derive(Clone, Debug)]
pub struct LayoutGlyph {
    /// Start index of cluster in original line
    pub start: usize,
    /// End index of cluster in original line
    pub end: usize,
    /// Font size of the glyph
    pub font_size: f32,
    /// Font weight of the glyph
    pub font_weight: fontdb::Weight,
    /// Line height of the glyph, will override buffer setting
    pub line_height_opt: Option<f32>,
    /// Font id of the glyph
    pub font_id: fontdb::ID,
    /// Font id of the glyph
    pub glyph_id: u16,
    /// X offset of hitbox
    pub x: f32,
    /// Y offset of hitbox
    pub y: f32,
    /// Width of hitbox
    pub w: f32,
    /// Unicode `BiDi` embedding level, character is left-to-right if `level` is divisible by 2
    pub level: unicode_bidi::Level,
    /// X offset in line
    ///
    /// If you are dealing with physical coordinates, use [`Self::physical`] to obtain a
    /// [`PhysicalGlyph`] for rendering.
    ///
    /// This offset is useful when you are dealing with logical units and you do not care or
    /// cannot guarantee pixel grid alignment. For instance, when you want to use the glyphs
    /// for vectorial text, apply linear transformations to the layout, etc.
    pub x_offset: f32,
    /// Y offset in line
    ///
    /// If you are dealing with physical coordinates, use [`Self::physical`] to obtain a
    /// [`PhysicalGlyph`] for rendering.
    ///
    /// This offset is useful when you are dealing with logical units and you do not care or
    /// cannot guarantee pixel grid alignment. For instance, when you want to use the glyphs
    /// for vectorial text, apply linear transformations to the layout, etc.
    pub y_offset: f32,
    /// Optional color override
    pub color_opt: Option<Color>,
    /// Metadata from `Attrs`
    pub metadata: usize,
    /// [`CacheKeyFlags`]
    pub cache_key_flags: CacheKeyFlags,
}

/// A span of consecutive glyphs sharing the same text decoration.
#[derive(Clone, Debug, PartialEq)]
pub struct DecorationSpan {
    /// Range of glyph indices in `LayoutLine::glyphs` covered by this span
    pub glyph_range: Range<usize>,
    /// The decoration config and metrics
    pub data: GlyphDecorationData,
    /// Fallback color from the first glyph's `color_opt`
    pub color_opt: Option<Color>,
    /// Font size from the first glyph (used to scale EM-unit metrics)
    pub font_size: f32,
}

#[derive(Clone, Debug)]
pub struct PhysicalGlyph {
    /// Cache key, see [`CacheKey`]
    pub cache_key: CacheKey,
    /// Integer component of X offset in line
    pub x: i32,
    /// Integer component of Y offset in line
    pub y: i32,
}

impl LayoutGlyph {
    pub fn physical(&self, offset: (f32, f32), scale: f32) -> PhysicalGlyph {
        let x_offset = self.font_size * self.x_offset;
        let y_offset = self.font_size * self.y_offset;

        let (cache_key, x, y) = CacheKey::new(
            self.font_id,
            self.glyph_id,
            self.font_size * scale,
            (
                (self.x + x_offset).mul_add(scale, offset.0),
                math::truncf((self.y - y_offset).mul_add(scale, offset.1)), // Hinting in Y axis
            ),
            self.font_weight,
            self.cache_key_flags,
        );

        PhysicalGlyph { cache_key, x, y }
    }
}

/// A line of laid out glyphs
#[derive(Clone, Debug)]
pub struct LayoutLine {
    /// Width of the line
    pub w: f32,
    /// Maximum ascent of the glyphs in line
    pub max_ascent: f32,
    /// Maximum descent of the glyphs in line
    pub max_descent: f32,
    /// Maximum line height of any spans in line that override the buffer's
    /// base line height in their `Attrs`, **including** that span's own
    /// top/bottom [`crate::SpanPadding`]
    ///
    /// A span's vertical padding belongs to the span's line box (its line
    /// height extended by the padding), so it only counts here for spans
    /// that override the line height; the padding of content laid out at
    /// the base line height is tracked in [`Self::base_pad`] instead.
    pub line_height_opt: Option<f32>,
    /// Whether the line contains any content laid out at the base line
    /// height, i.e. any glyph whose span does not override the line height
    /// in its `Attrs`
    ///
    /// When `true`, the base line height participates in determining the
    /// final line height via [`Self::line_height`]. When `false`, the
    /// line's content fully specifies its own line height (or the line is
    /// empty), so [`Self::line_height_opt`] alone determines it.
    pub uses_base_line_height: bool,
    /// Maximum top+bottom [`crate::SpanPadding`] (in pixels) applied to
    /// this line's content laid out at the base line height (spans that do
    /// not override the line height in their `Attrs`)
    ///
    /// Added to the base line height in [`Self::line_height`]. Unlike
    /// [`Self::top_pad`]/[`Self::bottom_pad`], which take the max over all
    /// of the line's spans and drive the baseline placement, this only
    /// covers the base-height content; the padding of spans that override
    /// the line height is included in [`Self::line_height_opt`] instead.
    pub base_pad: f32,
    /// Top [`crate::SpanPadding`] (in pixels) applied to this line: the max
    /// over all of the line's spans. Glyphs are placed below it, centered
    /// in the unpadded part of the line box, which shifts their baseline
    /// down.
    pub top_pad: f32,
    /// Bottom [`crate::SpanPadding`] (in pixels) applied to this line: the
    /// max over all of the line's spans. The line box extends below the
    /// glyphs by this much.
    pub bottom_pad: f32,
    /// Glyphs in line
    pub glyphs: Vec<LayoutGlyph>,
    /// Text decoration spans covering ranges of glyphs
    pub decorations: Vec<DecorationSpan>,
}

impl LayoutLine {
    /// The line's height given the buffer's base `line_height`, including
    /// this line's vertical [`crate::SpanPadding`].
    ///
    /// The height is the largest of:
    /// - the base line height plus the vertical padding of the line's
    ///   base-height content ([`Self::base_pad`]), when the line has any
    ///   such content ([`Self::uses_base_line_height`]);
    /// - the padded line height of each span that overrides the line height
    ///   in its `Attrs` ([`Self::line_height_opt`] holds the max).
    ///
    /// Spans with a line height larger than the base one increase the
    /// line's height. Spans with a smaller line height only reduce it when
    /// the line's content fully overrides the line height (e.g. a line that
    /// is entirely within such a span, or an empty line inside it);
    /// otherwise the base line height wins and the span has no impact on
    /// the line's height. Vertical padding extends its span's line box: it
    /// only grows the line beyond the base line height when the span's
    /// line height plus its padding exceeds it.
    pub const fn line_height(&self, line_height: f32) -> f32 {
        let base = if self.uses_base_line_height {
            line_height + self.base_pad
        } else {
            0.0
        };
        match self.line_height_opt {
            Some(span_line_height) => span_line_height.max(base),
            // No span overrides the line height: the base-height content
            // decides, which for an empty line means the buffer's base
            // line height.
            None => {
                if self.uses_base_line_height {
                    base
                } else {
                    line_height
                }
            }
        }
    }
}

/// Wrapping mode
#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum Wrap {
    /// No wrapping
    None,
    /// Wraps at a glyph level
    Glyph,
    /// Wraps at the word level
    Word,
    /// Wraps at the word level, or fallback to glyph level if a word can't fit on a line by itself
    WordOrGlyph,
}

impl Display for Wrap {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::None => write!(f, "No Wrap"),
            Self::Word => write!(f, "Word Wrap"),
            Self::WordOrGlyph => write!(f, "Word Wrap or Character"),
            Self::Glyph => write!(f, "Character"),
        }
    }
}

/// Align or justify
#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum Align {
    Left,
    Right,
    Center,
    Justified,
    End,
}

impl Display for Align {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Left => write!(f, "Left"),
            Self::Right => write!(f, "Right"),
            Self::Center => write!(f, "Center"),
            Self::Justified => write!(f, "Justified"),
            Self::End => write!(f, "End"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Ellipsize {
    /// No Ellipsizing
    #[default]
    None,
    /// Ellipsizes the start of the last visual line that fits within the `EllipsizeHeightLimit`
    Start(EllipsizeHeightLimit),
    /// Ellipsizes the middle of the last visual line that fits within the `EllipsizeHeightLimit`.
    Middle(EllipsizeHeightLimit),
    /// Ellipsizes the end of the last visual line that fits within the `EllipsizeHeightLimit`.
    End(EllipsizeHeightLimit),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EllipsizeHeightLimit {
    /// Number of lines to show before ellipsizing the rest. Only works if `Wrap` is NOT set to
    /// `Wrap::None`. Otherwise, it will be ignored and the behavior will be the same as `Lines(1)`
    Lines(usize),
    /// Ellipsizes the last line that fits within the given height limit. If `Wrap` is set to
    /// `Wrap::None`, the behavior will be the same as `Lines(1)`
    Height(f32),
}

/// Metrics hinting strategy
#[derive(Debug, Eq, PartialEq, Clone, Copy, Default)]
pub enum Hinting {
    /// No metrics hinting.
    ///
    /// Glyphs will have subpixel coordinates.
    ///
    /// This is the default.
    #[default]
    Disabled,

    /// Metrics hinting.
    ///
    /// Glyphs will be snapped to integral coordinates in the X-axis during layout.
    /// This can improve readability for smaller text and/or low-DPI screens.
    ///
    /// However, in order to get the right effect, you must use physical coordinates
    /// during layout and avoid further scaling when rendering. Otherwise, the rounding
    /// errors can accumulate and glyph distances may look erratic.
    ///
    /// In other words, metrics hinting makes layouting dependent of the target
    /// resolution.
    Enabled,
}
