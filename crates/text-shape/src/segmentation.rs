//! Unicode text segmentation: line breaks (UAX #14), graphemes (UAX #29)
//! and bidi runs (UAX #9). All offsets are UTF-8 byte offsets.

use std::ops::Range;

use unicode_bidi::{BidiInfo, Level};
use unicode_linebreak::BreakOpportunity;
use unicode_segmentation::UnicodeSegmentation;

use crate::shaping::Direction;

/// Paragraph base direction for bidi resolution.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "lowercase")
)]
pub enum BaseDirection {
    Ltr,
    Rtl,
    /// Detect from the first strong character.
    #[default]
    Auto,
}

impl BaseDirection {
    fn level(self) -> Option<Level> {
        match self {
            BaseDirection::Ltr => Some(Level::ltr()),
            BaseDirection::Rtl => Some(Level::rtl()),
            BaseDirection::Auto => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct LineBreak {
    /// A line may end right before this byte offset.
    pub index: usize,
    /// Hard break (newline etc.). The final entry is always a mandatory break at text end.
    pub mandatory: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct BidiRun {
    /// Byte range `[start, end)`.
    pub start: usize,
    pub end: usize,
    pub level: u8,
    /// `Ltr` or `Rtl`.
    pub direction: Direction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct BidiLine {
    /// `Ltr` or `Rtl`.
    pub base_direction: Direction,
    /// In visual order (left to right). Shape each run with its direction.
    pub runs: Vec<BidiRun>,
}

/// Line break opportunities (UAX #14).
pub fn line_breaks(text: &str) -> Vec<LineBreak> {
    unicode_linebreak::linebreaks(text)
        .map(|(index, op)| LineBreak {
            index,
            mandatory: op == BreakOpportunity::Mandatory,
        })
        .collect()
}

/// Grapheme cluster boundaries (UAX #29), including 0 and `text.len()`.
pub fn graphemes(text: &str) -> Vec<usize> {
    text.grapheme_indices(true)
        .map(|(i, _)| i)
        .chain(std::iter::once(text.len()))
        .collect()
}

/// Bidi analysis (UAX #9) of the line `line` (a byte range on char boundaries)
/// of `text`, as runs in visual order. Pass the whole paragraph as `text` so
/// embedding levels are resolved with full context.
pub fn bidi_line(text: &str, base: BaseDirection, line: Range<usize>) -> BidiLine {
    let line = line.start.min(text.len())..line.end.min(text.len());
    let info = BidiInfo::new(text, base.level());

    let base_level = info
        .paragraphs
        .iter()
        .find(|p| p.range.contains(&line.start))
        .or(info.paragraphs.last())
        .map(|p| p.level)
        .or(base.level())
        .unwrap_or_else(Level::ltr);

    let mut runs = Vec::new();
    // A line normally sits inside one paragraph; if it spans several, emit each
    // paragraph's runs in order.
    for para in &info.paragraphs {
        let start = para.range.start.max(line.start);
        let end = para.range.end.min(line.end);
        if start >= end {
            continue;
        }
        let (levels, level_runs) = info.visual_runs(para, start..end);
        runs.extend(level_runs.into_iter().map(|r| {
            let level = levels[r.start];
            BidiRun {
                start: r.start,
                end: r.end,
                level: level.number(),
                direction: dir(level),
            }
        }));
    }

    BidiLine {
        base_direction: dir(base_level),
        runs,
    }
}

fn dir(level: Level) -> Direction {
    if level.is_rtl() {
        Direction::Rtl
    } else {
        Direction::Ltr
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_breaks_after_spaces_and_newlines() {
        assert_eq!(
            line_breaks("hello world\nbye"),
            vec![
                LineBreak {
                    index: 6,
                    mandatory: false
                },
                LineBreak {
                    index: 12,
                    mandatory: true
                },
                LineBreak {
                    index: 15,
                    mandatory: true
                },
            ]
        );
    }

    #[test]
    fn graphemes_keep_emoji_sequences_together() {
        // The ZWJ family emoji is one grapheme.
        let text = "a👨‍👩‍👧b";
        assert_eq!(graphemes(text), vec![0, 1, text.len() - 1, text.len()]);
    }

    #[test]
    fn bidi_mixed_line_is_visually_ordered() {
        // "abc " + Hebrew "אבג" (2 bytes per letter) + " def"
        let text = "abc אבג def";
        let line = bidi_line(text, BaseDirection::Auto, 0..text.len());
        assert_eq!(line.base_direction, Direction::Ltr);
        let dirs: Vec<_> = line.runs.iter().map(|r| r.direction).collect();
        assert_eq!(dirs, vec![Direction::Ltr, Direction::Rtl, Direction::Ltr]);
        assert_eq!((line.runs[1].start, line.runs[1].end), (4, 10));

        let rtl_text = "אבג abc";
        let rtl = bidi_line(rtl_text, BaseDirection::Auto, 0..rtl_text.len());
        assert_eq!(rtl.base_direction, Direction::Rtl);
        // Visual order: the Latin run comes first (leftmost).
        assert_eq!(rtl.runs[0].direction, Direction::Ltr);
    }
}
