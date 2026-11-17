//! Text shaping with HarfRust.

use std::str::FromStr;

use harfrust::{Buffer, Feature, Language, Script, ShapeOptions as HrOptions, ShaperFont};

use crate::error::{Error, Result};
use crate::font::{Font, GlyphBBox};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "lowercase")
)]
pub enum Direction {
    Ltr,
    Rtl,
    Ttb,
    Btt,
}

impl Direction {
    fn to_hr(self) -> harfrust::Direction {
        match self {
            Direction::Ltr => harfrust::Direction::LeftToRight,
            Direction::Rtl => harfrust::Direction::RightToLeft,
            Direction::Ttb => harfrust::Direction::TopToBottom,
            Direction::Btt => harfrust::Direction::BottomToTop,
        }
    }

    fn from_hr(d: harfrust::Direction) -> Self {
        match d {
            harfrust::Direction::RightToLeft => Direction::Rtl,
            harfrust::Direction::TopToBottom => Direction::Ttb,
            harfrust::Direction::BottomToTop => Direction::Btt,
            _ => Direction::Ltr,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize), serde(default))]
pub struct ShapeOptions {
    /// Defaults to a guess from the text.
    pub direction: Option<Direction>,
    /// ISO 15924 tag, e.g. "Latn", "Arab". Defaults to a guess from the text.
    pub script: Option<String>,
    /// BCP 47 tag, e.g. "en", "tr".
    pub language: Option<String>,
    /// HarfBuzz feature syntax, e.g. "kern=0", "liga=0", "smcp", "ss01".
    pub features: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ShapedGlyph {
    pub id: u16,
    /// Byte offset of the first character this glyph came from.
    pub cluster: usize,
    /// How far to move the pen after this glyph, font units.
    pub x_advance: i32,
    /// Vertical advance; only used in vertical writing.
    pub y_advance: i32,
    /// Shift of this glyph from the pen position, font units.
    pub x_offset: i32,
    pub y_offset: i32,
    /// Ink bounds in font units, or `None` for empty glyphs such as spaces.
    pub bbox: Option<GlyphBBox>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ShapingResult {
    pub units_per_em: u16,
    /// Resolved direction. Glyphs are always in visual (left-to-right) order.
    pub direction: Direction,
    pub glyphs: Vec<ShapedGlyph>,
}

pub(crate) fn parse_features(features: &[String]) -> Result<Vec<Feature>> {
    features
        .iter()
        .map(|f| Feature::from_str(f).map_err(|e| Error::new(format!("{e}: {f:?}"))))
        .collect()
}

impl Font {
    /// Shapes a single run of text. For mixed-direction text, split it with
    /// [`bidi_line`](crate::bidi_line) first and shape each run with its direction,
    /// or use [`Font::layout`].
    pub fn shape(&self, text: &str, options: &ShapeOptions) -> Result<ShapingResult> {
        let features = parse_features(&options.features)?;
        self.shape_with(
            text,
            options.direction,
            options.script.as_deref(),
            options.language.as_deref(),
            &features,
        )
    }

    pub(crate) fn shape_with(
        &self,
        text: &str,
        direction: Option<Direction>,
        script: Option<&str>,
        language: Option<&str>,
        features: &[Feature],
    ) -> Result<ShapingResult> {
        let mut buf = Buffer::new();
        buf.push_str(text);
        if let Some(d) = direction {
            buf.set_direction(d.to_hr());
        }
        if let Some(s) = script {
            buf.set_script(Some(
                Script::from_str(s).map_err(|e| Error::new(format!("{e}: {s:?}")))?,
            ));
        }
        if let Some(l) = language {
            buf.set_language(Some(
                Language::from_str(l).map_err(|e| Error::new(format!("{e}: {l:?}")))?,
            ));
        }
        // Fills in whatever wasn't set explicitly.
        buf.guess_segment_properties();
        let direction = Direction::from_hr(buf.direction());

        let font = ShaperFont::new(self.instance());
        harfrust::shape(&font, &mut buf, HrOptions::new().features(features))
            .map_err(|e| Error::new(format!("could not shape text: {e}")))?;
        let glyphs = buf
            .glyph_infos()
            .iter()
            .zip(buf.glyph_positions())
            .map(|(info, pos)| {
                let id = info.glyph_id as u16;
                ShapedGlyph {
                    id,
                    cluster: (info.cluster as usize).min(text.len()),
                    x_advance: pos.x_advance,
                    y_advance: pos.y_advance,
                    x_offset: pos.x_offset,
                    y_offset: pos.y_offset,
                    bbox: self.glyph_bbox(id),
                }
            })
            .collect();

        Ok(ShapingResult {
            units_per_em: self.units_per_em(),
            direction,
            glyphs,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::test_font;

    #[test]
    fn shape_reports_clusters_bboxes_and_features() {
        let font = test_font();
        let r = font.shape("AV é", &ShapeOptions::default()).unwrap();
        assert_eq!(r.direction, Direction::Ltr);
        let clusters: Vec<_> = r.glyphs.iter().map(|g| g.cluster).collect();
        assert_eq!(clusters, vec![0, 1, 2, 3], "byte offsets");
        assert!(r.glyphs[0].bbox.is_some());
        assert!(r.glyphs[2].bbox.is_none(), "space has no ink");

        let kerned: i32 = r.glyphs[..2].iter().map(|g| g.x_advance).sum();
        let opts = ShapeOptions {
            features: vec!["kern=0".into()],
            ..Default::default()
        };
        let unkerned: i32 = font
            .shape("AV", &opts)
            .unwrap()
            .glyphs
            .iter()
            .map(|g| g.x_advance)
            .sum();
        assert!(
            kerned < unkerned,
            "AV should kern tighter ({kerned} vs {unkerned})"
        );

        let bad = ShapeOptions {
            features: vec!["???".into()],
            ..Default::default()
        };
        assert!(font.shape("a", &bad).is_err());
    }

    #[test]
    fn arabic_shapes_right_to_left_with_joining_forms() {
        let font = crate::font::load_test_font("NotoSansArabic.ttf");
        let word = font.shape("سلام", &ShapeOptions::default()).unwrap();
        assert_eq!(
            word.direction,
            Direction::Rtl,
            "direction guessed from the script"
        );
        assert!(word.glyphs.iter().all(|g| g.id != 0), "no missing glyphs");
        let clusters: Vec<_> = word.glyphs.iter().map(|g| g.cluster).collect();
        assert_eq!(
            clusters,
            vec![6, 4, 2, 0],
            "visual order: last letter first"
        );

        // The first letter, seen, joins to the next one: its initial form differs
        // from the isolated letter.
        let isolated = font.shape("س", &ShapeOptions::default()).unwrap().glyphs[0].id;
        let initial = word.glyphs.last().unwrap().id;
        assert_ne!(initial, isolated);
    }

    #[test]
    fn clusters_are_byte_offsets_after_multibyte_chars() {
        let font = test_font();
        let r = font.shape("éa", &ShapeOptions::default()).unwrap();
        assert_eq!(r.glyphs.last().unwrap().cluster, 2);
    }
}
