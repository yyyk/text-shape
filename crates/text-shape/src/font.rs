//! Fonts: loading, metrics, variation axes and style resolution.

use std::collections::BTreeMap;

use harfrust::Tag;
use read_fonts::TableProvider;
use read_fonts::types::{GlyphId, NameId};

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
    /// The font at its default axis values.
    base: harfrust::Font,
    /// The font at the current axis values (`base` for a static font).
    instance: harfrust::Font,
    /// The current value of every axis, in `base`'s axis order.
    values: Vec<(Tag, f32)>,
    /// Which face of a collection this is.
    face_index: u32,
}

impl Font {
    /// Parses a TTF/OTF, or face `face_index` of a TTC.
    pub fn from_bytes(data: impl Into<Vec<u8>>, face_index: u32) -> Result<Self> {
        let err = || Error::new(format!("could not parse font (face index {face_index})"));
        let base = harfrust::Font::new(data.into(), face_index).ok_or_else(err)?;
        // A font needs at least these to be drawn and shaped.
        let tables = base.tables();
        if tables.head().is_err() || tables.maxp().is_err() {
            return Err(err());
        }
        let values = base
            .axes()
            .iter()
            .map(|a| (a.tag(), a.default_value()))
            .collect();
        let instance = base.default_instance();
        Ok(Self {
            base,
            instance,
            values,
            face_index,
        })
    }

    /// The font at its default axis values.
    pub(crate) fn base(&self) -> &harfrust::Font {
        &self.base
    }

    pub(crate) fn face_index(&self) -> u32 {
        self.face_index
    }

    /// The font at the current axis values.
    pub(crate) fn instance(&self) -> &harfrust::Font {
        &self.instance
    }

    pub fn units_per_em(&self) -> u16 {
        self.instance.units_per_em()
    }

    /// Vertical metrics and basic info, in font units. Reflects current variations.
    pub fn metrics(&self) -> FontMetrics {
        let font = &self.instance;
        let tables = font.tables();
        let metrics = font.metrics();
        let style = font.style_metrics();
        let os2 = tables.os2().ok();
        let post = tables.post().ok();
        // As FreeType does: OS/2's typographic metrics when it asks for them,
        // otherwise hhea's.
        let line = if metrics.use_typo_metrics {
            metrics.typo_line.or(metrics.hhea_line)
        } else {
            metrics.hhea_line.or(metrics.typo_line)
        };
        let units = |v: read_fonts::types::F48Dot16| v.to_f32().round() as i16;
        let family_name = [NameId::TYPOGRAPHIC_FAMILY_NAME, NameId::FAMILY_NAME]
            .iter()
            .find_map(|&id| {
                font.names()
                    .get(id)?
                    .english_or_first()
                    .map(|n| n.name().chars().collect::<String>())
            });
        let selection = os2.as_ref().map(|t| t.fs_selection());
        let italic_angle = post.as_ref().map_or(0.0, |p| p.italic_angle().to_f32());
        let global_bbox = tables.head().map_or(
            GlyphBBox {
                x_min: 0,
                y_min: 0,
                x_max: 0,
                y_max: 0,
            },
            |h| GlyphBBox {
                x_min: h.x_min(),
                y_min: h.y_min(),
                x_max: h.x_max(),
                y_max: h.y_max(),
            },
        );
        FontMetrics {
            units_per_em: self.units_per_em(),
            ascender: line.map_or(0, |l| units(l.ascender)),
            descender: line.map_or(0, |l| units(l.descender)),
            line_gap: line.map_or(0, |l| units(l.line_gap)),
            cap_height: metrics.cap_height.map(units),
            x_height: metrics.x_height.map(units),
            underline_position: style.underline.map(|u| units(u.position)),
            underline_thickness: style.underline.map(|u| units(u.thickness)),
            italic_angle,
            weight: os2.as_ref().map_or(400, |t| t.us_weight_class()),
            // An italic angle also makes a font italic.
            is_italic: selection
                .is_some_and(|s| s.contains(read_fonts::tables::os2::SelectionFlags::ITALIC))
                || italic_angle != 0.0,
            is_bold: selection
                .is_some_and(|s| s.contains(read_fonts::tables::os2::SelectionFlags::BOLD)),
            is_monospaced: post.as_ref().is_some_and(|p| p.is_fixed_pitch() != 0),
            is_variable: !self.base.axes().is_empty(),
            family_name,
            global_bbox,
        }
    }

    /// Variation axes of a variable font (empty for static fonts).
    pub fn variation_axes(&self) -> Vec<VariationAxis> {
        self.base
            .axes()
            .iter()
            .map(|a| VariationAxis {
                tag: String::from_utf8_lossy(&a.tag().to_be_bytes()).into_owned(),
                min: a.min_value(),
                default: a.default_value(),
                max: a.max_value(),
                hidden: a.is_hidden(),
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
        let mut updated = self.values.clone();
        for &(tag, value) in values {
            let index = axes
                .iter()
                .position(|a| a.tag == tag)
                .ok_or_else(|| Error::new(format!("font has no {tag:?} variation axis")))?;
            let axis = &axes[index];
            updated[index].1 = value.clamp(axis.min, axis.max);
        }
        self.apply(updated);
        Ok(())
    }

    /// Resets every axis to its default value.
    pub fn reset_variations(&mut self) {
        let defaults = self
            .base
            .axes()
            .iter()
            .map(|a| (a.tag(), a.default_value()))
            .collect();
        self.apply(defaults);
    }

    fn apply(&mut self, values: Vec<(Tag, f32)>) {
        self.instance = self
            .base
            .instance_builder()
            .variations(values.iter().copied())
            .build();
        self.values = values;
    }

    /// Ink bounds of a glyph in font units, or `None` for empty glyphs.
    pub fn glyph_bbox(&self, glyph_id: u16) -> Option<GlyphBBox> {
        let e = self
            .instance
            .glyph_metrics()
            .extents(GlyphId::new(glyph_id.into()))?;
        if e.width == 0.0 && e.height == 0.0 {
            return None;
        }
        let r = |v: f32| v.round() as i16;
        Some(GlyphBBox {
            x_min: r(e.x_bearing),
            y_min: r(e.y_bearing - e.height),
            x_max: r(e.x_bearing + e.width),
            y_max: r(e.y_bearing),
        })
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
    fn variation_axes_report_the_hidden_flag() {
        // Roboto Flex marks its parametric axes hidden in fvar (flag 0x0001).
        let font = load_test_font("RobotoFlex.ttf");
        let hidden: Vec<_> = font
            .variation_axes()
            .into_iter()
            .filter(|a| a.hidden)
            .map(|a| a.tag)
            .collect();
        assert_eq!(
            hidden,
            [
                "XOPQ", "YOPQ", "XTRA", "YTUC", "YTLC", "YTAS", "YTDE", "YTFI"
            ]
        );
        assert!(!font.axis("wght").unwrap().hidden);
    }

    #[test]
    fn kerning_follows_the_font_variations() {
        // Total advances, kerning included, as HarfBuzz gives them (measured in
        // Chrome at 2048 px, Roboto Flex's units per em). Kerned pairs such as
        // "AV" and "To" change with the axes.
        const TEXT: &str = "AV To Wa Ye LT P. VA. 1234567890 ½ ¾ ‰";
        let mut font = load_test_font("RobotoFlex.ttf");
        assert_eq!(advance(&font, TEXT), 38635);
        font.set_variations(&[("wght", 100.0)]).unwrap();
        assert_eq!(advance(&font, TEXT), 38451);
        font.set_variations(&[("wght", 1000.0)]).unwrap();
        assert_eq!(advance(&font, TEXT), 41924);
        font.set_variations(&[("wdth", 25.0), ("opsz", 144.0)])
            .unwrap();
        assert_eq!(advance(&font, TEXT), 33560, "wght stays at 1000");
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
