//! Fonts: loading, metrics, variation axes and style resolution.

use std::collections::BTreeMap;

use rustybuzz::Variation;
use ttf_parser::Tag;

use crate::error::{Error, Result};

/// Stroke width used for faux bold, as a fraction of the em.
pub const SYNTHETIC_BOLD_EM: f32 = 1.0 / 24.0;
/// Slant used for faux italics, matching browsers.
pub const SYNTHETIC_OBLIQUE_DEGREES: f32 = 14.0;

/// A parsed font face that owns its data.
///
/// Variation axis values are part of the font's state: `set_variations` affects
/// later `metrics`, `shape`, `outline` and `layout` calls.
pub struct Font {
    // Borrows from `_data`. Declared first so it is dropped first. Never handed
    // out with its 'static lifetime.
    face: rustybuzz::Face<'static>,
    _data: Box<[u8]>,
}

impl Font {
    /// Parses a TTF/OTF, or face `face_index` of a TTC.
    pub fn from_bytes(data: impl Into<Vec<u8>>, face_index: u32) -> Result<Self> {
        let data = data.into().into_boxed_slice();
        // SAFETY: the boxed slice's heap allocation never moves or changes while
        // `data` is alive, and `face` (the only borrower) is dropped before it.
        let slice: &'static [u8] = unsafe { std::slice::from_raw_parts(data.as_ptr(), data.len()) };
        let face = rustybuzz::Face::from_slice(slice, face_index)
            .ok_or_else(|| Error::new(format!("could not parse font (face index {face_index})")))?;
        Ok(Self { face, _data: data })
    }

    pub(crate) fn face(&self) -> &rustybuzz::Face<'_> {
        &self.face
    }

    pub fn units_per_em(&self) -> u16 {
        self.face.units_per_em() as u16
    }

    /// Vertical metrics and basic info, in font units. Reflects current variations.
    pub fn metrics(&self) -> FontMetrics {
        let face = &self.face;
        let underline = face.underline_metrics();
        let family_name = [
            ttf_parser::name_id::TYPOGRAPHIC_FAMILY,
            ttf_parser::name_id::FAMILY,
        ]
        .iter()
        .find_map(|&id| {
            face.names()
                .into_iter()
                .filter(|n| n.name_id == id && n.is_unicode())
                .find_map(|n| n.to_string())
        });
        FontMetrics {
            units_per_em: self.units_per_em(),
            ascender: face.ascender(),
            descender: face.descender(),
            line_gap: face.line_gap(),
            cap_height: face.capital_height(),
            x_height: face.x_height(),
            underline_position: underline.map(|u| u.position),
            underline_thickness: underline.map(|u| u.thickness),
            italic_angle: face.italic_angle(),
            weight: face.weight().to_number(),
            is_italic: face.is_italic(),
            is_bold: face.is_bold(),
            is_monospaced: face.is_monospaced(),
            is_variable: face.is_variable(),
            family_name,
            global_bbox: face.global_bounding_box().into(),
        }
    }

    /// Variation axes of a variable font (empty for static fonts).
    pub fn variation_axes(&self) -> Vec<VariationAxis> {
        self.face
            .variation_axes()
            .into_iter()
            .map(|a| VariationAxis {
                tag: String::from_utf8_lossy(&a.tag.to_bytes()).into_owned(),
                min: a.min_value,
                default: a.def_value,
                max: a.max_value,
                hidden: a.hidden,
            })
            .collect()
    }

    pub fn axis(&self, tag: &str) -> Option<VariationAxis> {
        self.variation_axes().into_iter().find(|a| a.tag == tag)
    }

    /// Sets axis values, e.g. `[("wght", 700.0)]`, clamped to each axis range.
    /// Unlisted axes keep their current value. Errors on an unknown axis.
    pub fn set_variations(&mut self, values: &[(&str, f32)]) -> Result<()> {
        let axes = self.variation_axes();
        let mut variations = Vec::with_capacity(values.len());
        for &(tag, value) in values {
            let axis = axes
                .iter()
                .find(|a| a.tag == tag)
                .ok_or_else(|| Error::new(format!("font has no {tag:?} variation axis")))?;
            variations.push(Variation {
                tag: Tag::from_bytes_lossy(tag.as_bytes()),
                value: value.clamp(axis.min, axis.max),
            });
        }
        self.face.set_variations(&variations);
        Ok(())
    }

    /// Resets every axis to its default value.
    pub fn reset_variations(&mut self) {
        let defaults: Vec<Variation> = self
            .face
            .variation_axes()
            .into_iter()
            .map(|a| Variation {
                tag: a.tag,
                value: a.def_value,
            })
            .collect();
        self.face.set_variations(&defaults);
    }

    /// Ink bounds of a glyph in font units, or `None` for empty glyphs.
    pub fn glyph_bbox(&self, glyph_id: u16) -> Option<GlyphBBox> {
        self.face
            .glyph_bounding_box(ttf_parser::GlyphId(glyph_id))
            .map(Into::into)
    }

    /// Maps a requested weight/style onto this font: real axes (`wght`, `ital`,
    /// `slnt`) when it has them, otherwise faux bold/italic parameters.
    /// Apply the result with `set_variations` and draw with `synthetic`.
    pub fn resolve_style(&self, style: &FontStyle) -> ResolvedStyle {
        let metrics = self.metrics();
        let mut variations = BTreeMap::new();
        let mut synthetic = Synthetic::default();

        if self.axis("wght").is_some() {
            let weight = style.weight.unwrap_or(400.0);
            // Bold means at least 700, so it shows even with a light weight set.
            let weight = if style.bold {
                weight.max(700.0)
            } else {
                weight
            };
            variations.insert("wght".to_string(), weight);
        } else if style.bold && !metrics.is_bold {
            synthetic.bold = f32::from(metrics.units_per_em) * SYNTHETIC_BOLD_EM;
        }

        if style.italic {
            if self.axis("ital").is_some() {
                variations.insert("ital".to_string(), 1.0);
            } else if let Some(slnt) = self.axis("slnt") {
                variations.insert("slnt".to_string(), slnt.min.max(-12.0));
            } else if !metrics.is_italic {
                synthetic.oblique = SYNTHETIC_OBLIQUE_DEGREES.to_radians().tan();
            }
        }

        ResolvedStyle {
            variations,
            synthetic,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct GlyphBBox {
    pub x_min: i16,
    pub y_min: i16,
    pub x_max: i16,
    pub y_max: i16,
}

impl From<ttf_parser::Rect> for GlyphBBox {
    fn from(r: ttf_parser::Rect) -> Self {
        Self {
            x_min: r.x_min,
            y_min: r.y_min,
            x_max: r.x_max,
            y_max: r.y_max,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct FontMetrics {
    pub units_per_em: u16,
    pub ascender: i16,
    pub descender: i16,
    pub line_gap: i16,
    pub cap_height: Option<i16>,
    pub x_height: Option<i16>,
    pub underline_position: Option<i16>,
    pub underline_thickness: Option<i16>,
    pub italic_angle: f32,
    pub weight: u16,
    pub is_italic: bool,
    pub is_bold: bool,
    pub is_monospaced: bool,
    pub is_variable: bool,
    pub family_name: Option<String>,
    pub global_bbox: GlyphBBox,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct VariationAxis {
    pub tag: String,
    pub min: f32,
    pub default: f32,
    pub max: f32,
    pub hidden: bool,
}

/// A requested weight and style.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize), serde(default))]
pub struct FontStyle {
    /// `wght` value for variable fonts; defaults to 400. With `bold`, the weight
    /// is at least 700.
    pub weight: Option<f32>,
    pub bold: bool,
    pub italic: bool,
}

/// Faux styling to apply when drawing, for fonts without a real bold/italic.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Synthetic {
    /// Stroke width in font units to embolden with (stroke the glyph path); 0 = none.
    pub bold: f32,
    /// Horizontal skew for a faux italic: shift x right by `oblique` × height
    /// above the baseline; 0 = none.
    pub oblique: f32,
}

#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ResolvedStyle {
    /// Axis values to pass to `Font::set_variations`.
    pub variations: BTreeMap<String, f32>,
    pub synthetic: Synthetic,
}

/// Loads a font from the repository's `fonts/` folder.
#[cfg(test)]
pub(crate) fn load_test_font(name: &str) -> Font {
    let path = format!("{}/../../fonts/{name}", env!("CARGO_MANIFEST_DIR"));
    Font::from_bytes(
        std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}")),
        0,
    )
    .unwrap()
}

/// Static Roboto Regular.
#[cfg(test)]
pub(crate) fn test_font() -> Font {
    load_test_font("Roboto-Regular.ttf")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_and_static_font_variations() {
        let mut font = test_font();
        let m = font.metrics();
        assert!(m.ascender > 0 && m.descender < 0);
        assert!(m.cap_height.is_some());
        assert!(!m.is_variable);
        assert!(font.set_variations(&[("wght", 700.0)]).is_err());
    }

    #[test]
    fn static_font_gets_faux_bold_and_italic() {
        let font = test_font();
        let r = font.resolve_style(&FontStyle {
            bold: true,
            italic: true,
            ..Default::default()
        });
        assert!(r.variations.is_empty());
        assert!((r.synthetic.bold - 2048.0 / 24.0).abs() < 1e-3);
        assert!((r.synthetic.oblique - 0.2493).abs() < 1e-3);
        assert_eq!(
            font.resolve_style(&FontStyle::default()).synthetic,
            Synthetic::default()
        );
    }

    fn advance(font: &Font, text: &str) -> i32 {
        font.shape(text, &Default::default())
            .unwrap()
            .glyphs
            .iter()
            .map(|g| g.x_advance)
            .sum()
    }

    #[test]
    fn variable_font_axes_change_shaping_and_outlines() {
        let mut font = load_test_font("RobotoFlex.ttf");
        assert!(font.metrics().is_variable);
        let wght = font.axis("wght").expect("wght axis");
        assert_eq!((wght.min, wght.default, wght.max), (100.0, 400.0, 1000.0));

        let regular_width = advance(&font, "Hamburg");
        let regular_outline =
            font.outline(font.shape("H", &Default::default()).unwrap().glyphs[0].id);

        font.set_variations(&[("wght", 900.0)]).unwrap();
        assert!(
            advance(&font, "Hamburg") > regular_width,
            "heavier is wider"
        );
        let heavy_outline =
            font.outline(font.shape("H", &Default::default()).unwrap().glyphs[0].id);
        assert_ne!(heavy_outline, regular_outline, "outlines follow the axis");

        // Out-of-range values are clamped rather than rejected.
        font.set_variations(&[("wght", 5000.0)]).unwrap();
        let clamped = advance(&font, "Hamburg");
        font.set_variations(&[("wght", 1000.0)]).unwrap();
        assert_eq!(clamped, advance(&font, "Hamburg"));

        font.reset_variations();
        assert_eq!(advance(&font, "Hamburg"), regular_width);
    }

    #[test]
    fn variable_font_resolves_style_to_real_axes() {
        let font = load_test_font("RobotoFlex.ttf");
        let bold_italic = font.resolve_style(&FontStyle {
            bold: true,
            italic: true,
            ..Default::default()
        });
        assert_eq!(bold_italic.variations.get("wght"), Some(&700.0));
        assert_eq!(
            bold_italic.variations.get("slnt"),
            Some(&-10.0),
            "slnt min is -10"
        );
        assert_eq!(
            bold_italic.synthetic,
            Synthetic::default(),
            "no faux styling needed"
        );

        let weight = |weight: f32, bold: bool| {
            let style = FontStyle {
                weight: Some(weight),
                bold,
                ..Default::default()
            };
            font.resolve_style(&style).variations["wght"]
        };
        assert_eq!(
            weight(300.0, true),
            700.0,
            "bold raises a light weight to 700"
        );
        assert_eq!(weight(900.0, true), 900.0, "and keeps a heavier one");
        assert_eq!(weight(300.0, false), 300.0);
        assert_eq!(
            font.resolve_style(&FontStyle::default())
                .variations
                .get("wght"),
            Some(&400.0)
        );
    }
}
