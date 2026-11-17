//! Paragraph layout: hard breaks, wrapping, bidi reordering, alignment and bounds.

use std::collections::HashMap;

use harfrust::Feature;

use crate::error::Result;
use crate::font::{Font, GlyphBBox};
use crate::segmentation::{BaseDirection, BidiRun, bidi_line, graphemes, line_breaks};
use crate::shaping::{Direction, ShapedGlyph, parse_features};

const LINE_TERMINATORS: &[char] = &[
    '\r', '\n', '\u{B}', '\u{C}', '\u{85}', '\u{2028}', '\u{2029}',
];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "lowercase")
)]
pub enum WrapMode {
    /// Break at UAX #14 line break opportunities.
    #[default]
    Word,
    /// Break between any two grapheme clusters.
    Char,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "lowercase")
)]
pub enum HorizAlign {
    /// The side lines start from: left for left-to-right paragraphs, right for
    /// right-to-left ones. Like CSS and Canvas `text-align: start`.
    #[default]
    Start,
    /// The opposite side: right for left-to-right paragraphs, left for
    /// right-to-left ones.
    End,
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "lowercase")
)]
pub enum VertAlign {
    Top,
    Center,
    Bottom,
    #[default]
    Baseline,
}

/// How to lay out text. Units are px.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize), serde(default))]
pub struct LayoutOptions {
    /// Anchor point; its meaning depends on the alignment.
    pub x: f32,
    pub y: f32,
    pub size: f32,
    /// Distance between baselines. Defaults to 1.25 × `size`.
    pub leading: Option<f32>,
    /// Added after every cluster.
    pub letter_spacing: f32,
    /// Added after every space.
    pub word_spacing: f32,
    /// Extra space between specific pairs of characters.
    pub kerning: Vec<Kerning>,
    /// HarfBuzz feature syntax, e.g. "kern=0", "smcp". A range applies a feature
    /// to part of the text only: "kern[3:5]=0" (byte offsets into the text).
    pub features: Vec<String>,
    pub direction: BaseDirection,
    /// Only used when `max_width` is set.
    pub wrap: WrapMode,
    pub align_h: HorizAlign,
    pub align_v: VertAlign,
    /// Wrap lines to this width, and align horizontally within it.
    pub max_width: Option<f32>,
    /// Align vertically within this height. Text is not clipped.
    pub max_height: Option<f32>,
}

impl Default for LayoutOptions {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            size: 12.0,
            leading: None,
            letter_spacing: 0.0,
            word_spacing: 0.0,
            kerning: Vec::new(),
            features: Vec::new(),
            direction: BaseDirection::Auto,
            wrap: WrapMode::Word,
            align_h: HorizAlign::Start,
            align_v: VertAlign::Baseline,
            max_width: None,
            max_height: None,
        }
    }
}

/// Extra space between the character at `at` and the next one, on top of the
/// font's kerning and `letter_spacing`.
///
/// For right-to-left text the next character is the next one in reading order,
/// so the space appears on its left. Ignored for the last character of a line or
/// of a direction run. Inside a ligature, it applies after the whole ligature.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub struct Kerning {
    /// Byte offset of the first character of the pair.
    pub at: usize,
    /// px; negative values tighten.
    pub px: f32,
}

/// A rectangle in px (y down).
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Bounds {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct PositionedGlyph {
    pub id: u16,
    /// Byte offset into the input text.
    pub cluster: usize,
    /// Pen position in px at the baseline (y down).
    pub x: f32,
    pub y: f32,
    /// Ink bounds in font units (y up), or `None` for empty glyphs.
    pub bbox: Option<GlyphBBox>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct LayoutLine {
    /// Byte range of the line's visible text (trailing whitespace excluded).
    pub start: usize,
    pub end: usize,
    /// Left edge of the line's advance box, px.
    pub x: f32,
    pub baseline: f32,
    /// Advance width of the line, px.
    pub width: f32,
    /// Base direction of the line's paragraph: `Ltr` or `Rtl`.
    pub direction: Direction,
    pub glyphs: Vec<PositionedGlyph>,
    /// Tight box around the line's ink, or `None` if it has none.
    pub ink: Option<Bounds>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct TextLayout {
    /// Always at least one line.
    pub lines: Vec<LayoutLine>,
    /// px per font unit
    pub scale: f32,
    /// Font ascent and descent in px, both positive.
    pub ascent: f32,
    pub descent: f32,
    /// Loose box: each line's advance width × font ascent/descent.
    pub advance: Bounds,
    /// Tight box around all ink, or `None` if there is none.
    pub ink: Option<Bounds>,
}

impl TextLayout {
    /// Advance width of the widest line.
    pub fn width(&self) -> f32 {
        self.lines.iter().map(|l| l.width).fold(0.0, f32::max)
    }

    /// Ink width of the widest line.
    pub fn ink_width(&self) -> f32 {
        self.lines
            .iter()
            .filter_map(|l| l.ink)
            .map(|b| b.w)
            .fold(0.0, f32::max)
    }
}

struct LineRange<'a> {
    para: &'a str,
    para_start: usize,
    start: usize,
    end: usize,
}

impl Font {
    /// Lays out `text`. Hard breaks always start a new line; with `max_width`,
    /// lines also wrap per `options.wrap`. Uses the font's current variations.
    pub fn layout(&self, text: &str, options: &LayoutOptions) -> Result<TextLayout> {
        let metrics = self.metrics();
        let scale = options.size / f32::from(metrics.units_per_em);
        let ascent = f32::from(metrics.ascender) * scale;
        let descent = -f32::from(metrics.descender) * scale;
        let leading = options.leading.unwrap_or(options.size * 1.25);
        let features = parse_features(&options.features)?;

        // 1. Break into lines (logical ranges, trailing whitespace trimmed).
        let mut ranges = Vec::new();
        for (para_start, para) in paragraphs(text) {
            let line_ranges = match options.max_width {
                None => vec![(0, para.len())],
                Some(max_width) => {
                    self.wrap_paragraph(para, para_start, max_width, options, &features, scale)?
                }
            };
            for (start, end) in line_ranges {
                ranges.push(LineRange {
                    para,
                    para_start,
                    start,
                    end: trim_end(para, start, end),
                });
            }
        }

        // 2. Shape each line in visual order.
        let mut lines = Vec::with_capacity(ranges.len());
        for (i, r) in ranges.iter().enumerate() {
            let bidi = bidi_line(r.para, options.direction, r.start..r.end);
            let mut glyphs = Vec::new();
            let mut pen = 0.0f64;
            for run in &bidi.runs {
                let placed =
                    self.place_run(r.para, r.para_start, run, options, &features, scale)?;
                for p in placed.glyphs {
                    glyphs.push(PositionedGlyph {
                        id: p.glyph.id,
                        cluster: r.para_start + run.start + p.glyph.cluster,
                        x: (pen + p.x) as f32,
                        y: -(p.glyph.y_offset as f32) * scale,
                        bbox: p.glyph.bbox,
                    });
                }
                pen += placed.width;
            }
            lines.push(LayoutLine {
                start: r.para_start + r.start,
                end: r.para_start + r.end,
                x: 0.0,
                baseline: i as f32 * leading,
                width: pen as f32,
                direction: bidi.base_direction,
                glyphs,
                ink: None,
            });
        }

        // 3. Align.
        let top = -ascent;
        let bottom = (lines.len().max(1) - 1) as f32 * leading + descent;
        let y = options.y;
        let dy = match (options.max_height, options.align_v) {
            (None, VertAlign::Baseline) => y,
            (None, VertAlign::Top) => y - top,
            (None, VertAlign::Center) => y - (top + bottom) / 2.0,
            (None, VertAlign::Bottom) => y - bottom,
            (Some(_), VertAlign::Baseline | VertAlign::Top) => y - top,
            (Some(h), VertAlign::Center) => y + h / 2.0 - (top + bottom) / 2.0,
            (Some(h), VertAlign::Bottom) => y + h - bottom,
        };
        let box_width = options.max_width.unwrap_or(0.0);
        for line in &mut lines {
            line.x = options.x
                + match options.align_h {
                    HorizAlign::Left => 0.0,
                    HorizAlign::Center => (box_width - line.width) / 2.0,
                    HorizAlign::Right => box_width - line.width,
                    // Start/end follow each line's paragraph direction.
                    HorizAlign::Start if line.direction == Direction::Rtl => box_width - line.width,
                    HorizAlign::End if line.direction != Direction::Rtl => box_width - line.width,
                    HorizAlign::Start | HorizAlign::End => 0.0,
                };
            line.baseline += dy;
            for g in &mut line.glyphs {
                g.x += line.x;
                g.y += line.baseline;
            }
            line.ink = ink_bounds(&line.glyphs, scale);
        }

        // 4. Bounds.
        let advance = union(lines.iter().map(|l| Bounds {
            x: l.x,
            y: l.baseline - ascent,
            w: l.width,
            h: ascent + descent,
        }))
        .unwrap_or(Bounds {
            x: options.x,
            y: options.y,
            w: 0.0,
            h: 0.0,
        });
        let ink = union(lines.iter().filter_map(|l| l.ink));

        Ok(TextLayout {
            lines,
            scale,
            ascent,
            descent,
            advance,
            ink,
        })
    }

    /// Greedy line fitting over the break candidates of `options.wrap`.
    fn wrap_paragraph(
        &self,
        para: &str,
        para_start: usize,
        max_width: f32,
        options: &LayoutOptions,
        features: &[Feature],
        scale: f32,
    ) -> Result<Vec<(usize, usize)>> {
        if para.is_empty() {
            return Ok(vec![(0, 0)]);
        }

        let prefix = self.advance_prefix(para, para_start, options, features, scale)?;
        let width = |s: usize, e: usize| prefix[e] - prefix[s];
        let fits = |s: usize, e: usize| width(s, trim_end(para, s, e)) <= f64::from(max_width);
        let candidates: Vec<usize> = match options.wrap {
            WrapMode::Char => graphemes(para).into_iter().skip(1).collect(),
            WrapMode::Word => line_breaks(para).into_iter().map(|b| b.index).collect(),
        };

        let mut lines = Vec::new();
        let mut start = 0;
        let mut last_fit = None;
        for c in candidates {
            if fits(start, c) {
                last_fit = Some(c);
                continue;
            }
            if let Some(fit) = last_fit.filter(|&f| f > start) {
                lines.push((start, fit));
                start = fit;
            }
            if fits(start, c) {
                last_fit = Some(c);
            } else {
                // A single unbreakable segment wider than the box: let it overflow.
                lines.push((start, c));
                start = c;
                last_fit = None;
            }
        }
        if start < para.len() {
            lines.push((start, para.len()));
        }
        Ok(lines)
    }

    /// Prefix sums of advance width (px) per byte offset of `para`, from one
    /// shaping pass over the paragraph. Ignores kerning/ligatures across range
    /// edges, which is fine for choosing break points.
    fn advance_prefix(
        &self,
        para: &str,
        para_start: usize,
        options: &LayoutOptions,
        features: &[Feature],
        scale: f32,
    ) -> Result<Vec<f64>> {
        let mut per_cluster = vec![0.0f64; para.len() + 1];
        for run in bidi_line(para, options.direction, 0..para.len()).runs {
            let placed = self.place_run(para, para_start, &run, options, features, scale)?;
            for (cluster, width) in placed.cluster_widths {
                per_cluster[run.start + cluster] += width;
            }
        }
        let mut prefix = vec![0.0f64; para.len() + 1];
        for i in 0..para.len() {
            prefix[i + 1] = prefix[i] + per_cluster[i];
        }
        Ok(prefix)
    }
}

/// One shaped direction run, placed from pen position 0.
struct PlacedRun {
    glyphs: Vec<PlacedGlyph>,
    /// Advance width including spacing and kerning, px.
    width: f64,
    /// Width attributed to each cluster (run-relative byte offset), for measuring
    /// logical ranges.
    cluster_widths: Vec<(usize, f64)>,
}

struct PlacedGlyph {
    /// Cluster is relative to the run.
    glyph: ShapedGlyph,
    /// Pen position plus the glyph's own offset, px from the run start.
    x: f64,
}

impl Font {
    /// Shapes one direction run of a paragraph and places its glyphs, adding
    /// letter/word spacing and manual kerning between clusters (and spacing after
    /// the last one). Spacing goes between clusters rather than after a cluster's
    /// first glyph, so marks stay attached to their base.
    fn place_run(
        &self,
        para: &str,
        para_start: usize,
        run: &BidiRun,
        options: &LayoutOptions,
        features: &[Feature],
        scale: f32,
    ) -> Result<PlacedRun> {
        let text = &para[run.start..run.end];
        let offset = para_start + run.start;
        let features = rebase_features(features, offset, text.len());
        let shaped = self.shape_with(text, Some(run.direction), None, None, &features)?;
        let kerning = kerning_by_cluster(&options.kerning, &shaped.glyphs, offset, text.len());
        let rtl = run.direction == Direction::Rtl;
        let scale = f64::from(scale);

        let mut glyphs = Vec::with_capacity(shaped.glyphs.len());
        let mut widths: Vec<(usize, f64)> = Vec::new();
        let mut add_width = |cluster: usize, w: f64| match widths.last_mut() {
            Some((c, total)) if *c == cluster => *total += w,
            _ => widths.push((cluster, w)),
        };
        let mut pen = 0.0f64;
        let mut prev: Option<usize> = None;
        for g in shaped.glyphs {
            let cluster = g.cluster;
            if let Some(p) = prev.filter(|&p| p != cluster) {
                // Between visually adjacent clusters `p` (left) and `cluster`
                // (right). Spacing belongs to the left one; kerning to whichever
                // comes first in reading order.
                let spacing = f64::from(spacing_after(text, p, options));
                let first = if rtl { cluster } else { p };
                let kern = kerning.get(&first).copied().unwrap_or(0.0);
                pen += spacing + kern;
                add_width(p, spacing);
                add_width(first, kern);
            }
            let advance = f64::from(g.x_advance) * scale;
            glyphs.push(PlacedGlyph {
                x: pen + f64::from(g.x_offset) * scale,
                glyph: g,
            });
            pen += advance;
            add_width(cluster, advance);
            prev = Some(cluster);
        }
        if let Some(p) = prev {
            let spacing = f64::from(spacing_after(text, p, options));
            pen += spacing;
            add_width(p, spacing);
        }
        Ok(PlacedRun {
            glyphs,
            width: pen,
            cluster_widths: widths,
        })
    }
}

/// Shifts feature ranges (byte offsets into the whole text) to a piece of it
/// that starts at `offset`, dropping features that don't overlap it.
fn rebase_features(features: &[Feature], offset: usize, len: usize) -> Vec<Feature> {
    let (start, end) = (offset as u32, (offset + len) as u32);
    features
        .iter()
        .filter(|f| f.start < end && f.end > start)
        .map(|f| Feature {
            start: f.start.saturating_sub(start),
            end: if f.end == u32::MAX {
                u32::MAX
            } else {
                f.end - start
            },
            ..*f
        })
        .collect()
}

/// Manual kerning for a run, keyed by the run-relative cluster containing each
/// pair's first character.
fn kerning_by_cluster(
    kerning: &[Kerning],
    glyphs: &[ShapedGlyph],
    offset: usize,
    len: usize,
) -> HashMap<usize, f64> {
    let mut out = HashMap::new();
    if kerning.is_empty() {
        return out;
    }
    let mut clusters: Vec<usize> = glyphs.iter().map(|g| g.cluster).collect();
    clusters.sort_unstable();
    clusters.dedup();
    for k in kerning
        .iter()
        .filter(|k| (offset..offset + len).contains(&k.at))
    {
        let at = k.at - offset;
        let containing = clusters[clusters.partition_point(|&c| c <= at).saturating_sub(1)];
        *out.entry(containing).or_insert(0.0) += f64::from(k.px);
    }
    out
}

/// Splits at hard breaks as `(start, paragraph)`; paragraphs exclude their
/// newline characters. A trailing newline still produces an empty last line.
fn paragraphs(text: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut start = 0;
    for b in line_breaks(text) {
        if !b.mandatory {
            continue;
        }
        out.push((
            start,
            text[start..b.index].trim_end_matches(LINE_TERMINATORS),
        ));
        start = b.index;
    }
    if start < text.len() || out.is_empty() || text.ends_with(LINE_TERMINATORS) {
        out.push((start, text[start..].trim_end_matches(LINE_TERMINATORS)));
    }
    out
}

fn spacing_after(para: &str, cluster: usize, options: &LayoutOptions) -> f32 {
    let space = para.as_bytes().get(cluster) == Some(&b' ');
    options.letter_spacing + if space { options.word_spacing } else { 0.0 }
}

fn trim_end(para: &str, start: usize, end: usize) -> usize {
    start + para[start..end].trim_end().len()
}

fn ink_bounds(glyphs: &[PositionedGlyph], scale: f32) -> Option<Bounds> {
    union(glyphs.iter().filter_map(|g| {
        let b = g.bbox?;
        let x = g.x + f32::from(b.x_min) * scale;
        let y = g.y - f32::from(b.y_max) * scale;
        Some(Bounds {
            x,
            y,
            w: f32::from(b.x_max - b.x_min) * scale,
            h: f32::from(b.y_max - b.y_min) * scale,
        })
    }))
}

fn union(boxes: impl Iterator<Item = Bounds>) -> Option<Bounds> {
    boxes
        .map(|b| (b.x, b.y, b.x + b.w, b.y + b.h))
        .reduce(|a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)))
        .map(|(x0, y0, x1, y1)| Bounds {
            x: x0,
            y: y0,
            w: x1 - x0,
            h: y1 - y0,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::test_font;

    fn texts<'a>(text: &'a str, l: &TextLayout) -> Vec<&'a str> {
        l.lines
            .iter()
            .map(|line| &text[line.start..line.end])
            .collect()
    }

    fn opts(size: f32) -> LayoutOptions {
        LayoutOptions {
            size,
            ..Default::default()
        }
    }

    #[test]
    fn hard_breaks_blank_lines_and_leading() {
        let font = test_font();
        let l = font.layout("a\n\nb", &opts(10.0)).unwrap();
        assert_eq!(texts("a\n\nb", &l), vec!["a", "", "b"]);
        let baselines: Vec<_> = l.lines.iter().map(|l| l.baseline).collect();
        assert_eq!(
            baselines,
            vec![0.0, 12.5, 25.0],
            "default leading = 1.25 × size"
        );

        assert_eq!(
            font.layout("a\n", &opts(10.0)).unwrap().lines.len(),
            2,
            "trailing newline"
        );
        assert_eq!(
            font.layout("", &opts(10.0)).unwrap().lines.len(),
            1,
            "empty text"
        );
        assert_eq!(
            font.layout("a\r\nb", &opts(10.0)).unwrap().lines.len(),
            2,
            "CRLF is one break"
        );
    }

    #[test]
    fn word_wrap_fits_greedily_and_overflows_long_words() {
        let font = test_font();
        let text = "The quick brown fox jumps over the lazy dog";
        let max = 300.0;
        let l = font
            .layout(
                text,
                &LayoutOptions {
                    size: 48.0,
                    max_width: Some(max),
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(l.lines.len() > 1);
        for (i, line) in l.lines.iter().enumerate() {
            assert!(line.width <= max + 0.01, "line {i} fits");
            assert!(
                !text[line.start..line.end].ends_with(' '),
                "trailing space trimmed"
            );
            // Greedy: adding the next word would have overflowed.
            if let Some(next) = l.lines.get(i + 1) {
                let next_word_end = text[next.start..]
                    .find(' ')
                    .map_or(text.len(), |p| next.start + p);
                let longer = font
                    .layout(&text[line.start..next_word_end], &opts(48.0))
                    .unwrap();
                assert!(longer.width() > max, "line {i} is greedy");
            }
        }

        let l = font
            .layout(
                "Supercalifragilistic ok",
                &LayoutOptions {
                    size: 48.0,
                    max_width: Some(100.0),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(
            texts("Supercalifragilistic ok", &l),
            vec!["Supercalifragilistic", "ok"]
        );
    }

    #[test]
    fn char_wrap_splits_words() {
        let font = test_font();
        let o = LayoutOptions {
            size: 48.0,
            max_width: Some(100.0),
            wrap: WrapMode::Char,
            ..Default::default()
        };
        let l = font.layout("abcdefghij", &o).unwrap();
        assert!(l.lines.len() > 1);
        assert_eq!(texts("abcdefghij", &l).concat(), "abcdefghij");
        assert!(l.lines.iter().all(|line| line.width <= 100.01));
    }

    #[test]
    fn horizontal_and_vertical_alignment() {
        let font = test_font();
        let base = font.layout("ab", &opts(48.0)).unwrap();
        let w = base.lines[0].width;
        let at = |o: LayoutOptions| font.layout("ab", &o).unwrap().lines[0].clone();

        let c = at(LayoutOptions {
            size: 48.0,
            align_h: HorizAlign::Center,
            max_width: Some(200.0),
            ..Default::default()
        });
        assert!((c.x - (200.0 - w) / 2.0).abs() < 1e-3);
        let r = at(LayoutOptions {
            size: 48.0,
            align_h: HorizAlign::Right,
            x: 10.0,
            ..Default::default()
        });
        assert!((r.x - (10.0 - w)).abs() < 1e-3);

        let top = at(LayoutOptions {
            size: 48.0,
            align_v: VertAlign::Top,
            ..Default::default()
        });
        assert!(
            (top.baseline - base.ascent).abs() < 1e-3,
            "TOP puts the baseline at the ascent"
        );
        let two = font
            .layout(
                "a\nb",
                &LayoutOptions {
                    size: 48.0,
                    align_v: VertAlign::Bottom,
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(
            (two.lines[1].baseline + two.descent).abs() < 1e-3,
            "BOTTOM puts the last descent at y"
        );
    }

    #[test]
    fn start_and_end_follow_each_paragraphs_direction() {
        let font = crate::font::load_test_font("NotoSansArabic.ttf");
        // A left-to-right paragraph, then a right-to-left one, in a 400px box.
        let text = "abc\nسلام";
        let line_x = |align_h: HorizAlign| {
            let o = with(40.0, |o| {
                o.align_h = align_h;
                o.max_width = Some(400.0);
            });
            let l = font.layout(text, &o).unwrap();
            assert_eq!(
                (l.lines[0].direction, l.lines[1].direction),
                (Direction::Ltr, Direction::Rtl)
            );
            l.lines
                .iter()
                .map(|line| (line.x, 400.0 - line.width))
                .collect::<Vec<_>>()
        };
        let close = |a: f32, b: f32| (a - b).abs() < 1e-3;

        let start = line_x(HorizAlign::Start);
        assert!(close(start[0].0, 0.0), "start of an LTR line: left edge");
        assert!(
            close(start[1].0, start[1].1),
            "start of an RTL line: right edge"
        );

        let end = line_x(HorizAlign::End);
        assert!(close(end[0].0, end[0].1), "end of an LTR line: right edge");
        assert!(close(end[1].0, 0.0), "end of an RTL line: left edge");

        let left = line_x(HorizAlign::Left);
        assert!(
            close(left[0].0, 0.0) && close(left[1].0, 0.0),
            "left ignores direction"
        );
        assert_eq!(
            LayoutOptions::default().align_h,
            HorizAlign::Start,
            "start is the default"
        );
    }

    #[test]
    fn bounds_and_spacing() {
        let font = test_font();
        let l = font.layout("Hag", &opts(48.0)).unwrap();
        let ink = l.ink.unwrap();
        assert!(
            ink.y < 0.0 && ink.y + ink.h > 0.0,
            "ink spans the baseline (g descends)"
        );
        assert!(ink.w <= l.width());
        assert!((l.advance.h - (l.ascent + l.descent)).abs() < 1e-3);
        assert!(font.layout("   ", &opts(48.0)).unwrap().ink.is_none());

        let spaced = font
            .layout(
                "ab",
                &LayoutOptions {
                    size: 48.0,
                    letter_spacing: 2.0,
                    ..Default::default()
                },
            )
            .unwrap();
        let plain = font.layout("ab", &opts(48.0)).unwrap();
        assert!(
            (spaced.width() - plain.width() - 4.0).abs() < 1e-3,
            "2px after each of 2 clusters"
        );
    }

    #[test]
    fn mixed_direction_line_orders_runs_visually() {
        let font = crate::font::load_test_font("NotoSansArabic.ttf");
        // Latin glyphs are missing from this font, but their runs and clusters still place.
        let text = "ab سلام cd";
        let l = font.layout(text, &opts(40.0)).unwrap();
        let line = &l.lines[0];
        assert_eq!(
            line.direction,
            Direction::Ltr,
            "first strong character is Latin"
        );
        let clusters: Vec<_> = line.glyphs.iter().map(|g| g.cluster).collect();
        let arabic_start = text.find('س').unwrap();
        // "ab " left to right, then the Arabic word right to left, then " cd".
        assert_eq!(&clusters[..3], &[0, 1, 2]);
        assert_eq!(
            &clusters[3..7],
            &[
                arabic_start + 6,
                arabic_start + 4,
                arabic_start + 2,
                arabic_start
            ]
        );
        assert_eq!(
            &clusters[7..],
            &[arabic_start + 8, arabic_start + 9, arabic_start + 10]
        );
        let xs: Vec<_> = line.glyphs.iter().map(|g| g.x).collect();
        assert!(
            xs.windows(2).all(|w| w[0] <= w[1]),
            "glyphs come out left to right"
        );

        let rtl = font.layout("سلام ab", &opts(40.0)).unwrap();
        assert_eq!(
            rtl.lines[0].direction,
            Direction::Rtl,
            "first strong character is Arabic"
        );
    }

    fn with<F: FnOnce(&mut LayoutOptions)>(size: f32, f: F) -> LayoutOptions {
        let mut o = opts(size);
        f(&mut o);
        o
    }

    #[test]
    fn ranged_features_apply_to_their_range_in_any_paragraph() {
        let font = test_font();
        let text = "xx\nAV AV"; // the second paragraph's first "AV" is at bytes 3..5
        let line = |features: &[&str]| {
            let o = with(100.0, |o| {
                o.features = features.iter().map(|f| f.to_string()).collect()
            });
            font.layout(text, &o).unwrap().lines[1].clone()
        };
        let plain = line(&[]);
        let unkerned_pair = line(&["kern[3:5]=0"]);
        assert!(
            unkerned_pair.width > plain.width,
            "the range reaches the second paragraph"
        );
        // Only the first pair changed: its V moved, the second pair didn't change shape.
        let gap = |l: &LayoutLine, i: usize| l.glyphs[i + 1].x - l.glyphs[i].x;
        assert!(gap(&unkerned_pair, 0) > gap(&plain, 0));
        assert!((gap(&unkerned_pair, 3) - gap(&plain, 3)).abs() < 1e-3);

        // Wrapping measures with the same features.
        let all_off = with(100.0, |o| o.features = vec!["kern=0".into()]);
        let ranged = with(100.0, |o| o.features = vec!["kern[0:100]=0".into()]);
        assert_eq!(
            font.layout(text, &all_off).unwrap(),
            font.layout(text, &ranged).unwrap()
        );
    }

    #[test]
    fn manual_kerning_adds_space_between_a_pair() {
        let font = test_font();
        let plain = font.layout("AVA", &opts(100.0)).unwrap().lines[0].clone();
        let kerned = font
            .layout(
                "AVA",
                &with(100.0, |o| o.kerning = vec![Kerning { at: 0, px: 10.0 }]),
            )
            .unwrap()
            .lines[0]
            .clone();
        let dx: Vec<f32> = (0..3)
            .map(|i| kerned.glyphs[i].x - plain.glyphs[i].x)
            .collect();
        assert!(dx[0].abs() < 1e-3 && (dx[1] - 10.0).abs() < 1e-3 && (dx[2] - 10.0).abs() < 1e-3);
        assert!((kerned.width - plain.width - 10.0).abs() < 1e-3);

        // Negative values tighten; several entries for one pair add up.
        let tight = vec![Kerning { at: 1, px: -3.0 }, Kerning { at: 1, px: -2.0 }];
        let l = font
            .layout("AVA", &with(100.0, |o| o.kerning = tight))
            .unwrap();
        assert!((l.lines[0].width - plain.width + 5.0).abs() < 1e-3);

        // Ignored after the last character of a line.
        let last = with(100.0, |o| o.kerning = vec![Kerning { at: 2, px: 50.0 }]);
        assert_eq!(
            font.layout("AVA", &last).unwrap().lines[0].width,
            plain.width
        );
    }

    #[test]
    fn manual_kerning_uses_byte_offsets_and_whole_clusters() {
        let font = test_font();
        // "é" is 2 bytes; a pair index inside it means the same character.
        let plain = font.layout("éa", &opts(100.0)).unwrap().width();
        for at in [0, 1] {
            let o = with(100.0, |o| o.kerning = vec![Kerning { at, px: 7.0 }]);
            assert!((font.layout("éa", &o).unwrap().width() - plain - 7.0).abs() < 1e-3);
        }
        // Inside the "fi" ligature: applies after the ligature.
        let fi = font.layout("fix", &opts(100.0)).unwrap().lines[0].clone();
        assert_eq!(fi.glyphs.len(), 2, "fi is one glyph");
        let o = with(100.0, |o| o.kerning = vec![Kerning { at: 0, px: 9.0 }]);
        let kerned = font.layout("fix", &o).unwrap().lines[0].clone();
        assert!((kerned.glyphs[1].x - fi.glyphs[1].x - 9.0).abs() < 1e-3);
    }

    #[test]
    fn manual_kerning_in_right_to_left_text_goes_on_the_left() {
        let font = crate::font::load_test_font("NotoSansArabic.ttf");
        let text = "سلام"; // visual order: م ا ل س, clusters 6 4 2 0
        // Left-aligned, so a wider line doesn't move its left edge.
        let left = with(60.0, |o| o.align_h = HorizAlign::Left);
        let plain = font.layout(text, &left).unwrap().lines[0].clone();
        // Between the first two letters in reading order, س and ل.
        let o = with(60.0, |o| {
            o.align_h = HorizAlign::Left;
            o.kerning = vec![Kerning { at: 0, px: 12.0 }];
        });
        let kerned = font.layout(text, &o).unwrap().lines[0].clone();
        let dx: Vec<f32> = (0..4)
            .map(|i| kerned.glyphs[i].x - plain.glyphs[i].x)
            .collect();
        assert_eq!(kerned.glyphs[3].cluster, 0, "س is drawn last (rightmost)");
        assert!(
            dx[..3].iter().all(|d| d.abs() < 1e-3),
            "glyphs left of the gap stay put"
        );
        assert!(
            (dx[3] - 12.0).abs() < 1e-3,
            "س moves right: the gap opens on its left"
        );
    }

    #[test]
    fn letter_spacing_keeps_marks_on_their_base() {
        let font = test_font();
        // "x" + combining acute: no precomposed glyph, so two glyphs in one cluster.
        let text = "x\u{301}y";
        let mark_offset = |o: &LayoutOptions| {
            let line = font.layout(text, o).unwrap().lines[0].clone();
            assert_eq!(line.glyphs[0].cluster, line.glyphs[1].cluster);
            line.glyphs[1].x - line.glyphs[0].x
        };
        let spaced = with(100.0, |o| o.letter_spacing = 20.0);
        assert!((mark_offset(&spaced) - mark_offset(&opts(100.0))).abs() < 1e-3);
    }

    #[test]
    fn layout_uses_the_fonts_current_variations() {
        let mut font = crate::font::load_test_font("RobotoFlex.ttf");
        let regular = font.layout("Hamburg", &opts(48.0)).unwrap().width();
        font.set_variations(&[("wght", 900.0)]).unwrap();
        assert!(font.layout("Hamburg", &opts(48.0)).unwrap().width() > regular);
    }

    #[test]
    fn mixed_direction_line_and_byte_clusters() {
        let font = test_font();
        let text = "é abc";
        let l = font.layout(text, &opts(48.0)).unwrap();
        let clusters: Vec<_> = l.lines[0].glyphs.iter().map(|g| g.cluster).collect();
        assert_eq!(clusters, vec![0, 2, 3, 4, 5]);

        let rtl = font
            .layout(
                "abc",
                &LayoutOptions {
                    size: 48.0,
                    direction: BaseDirection::Rtl,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(rtl.lines[0].direction, Direction::Rtl);
    }
}
