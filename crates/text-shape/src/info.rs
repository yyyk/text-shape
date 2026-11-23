//! Font introspection: names, character coverage, glyph names, OpenType
//! features, named instances and the table directory. None of it depends on
//! the current variations.

use std::collections::{BTreeMap, BTreeSet};

use read_fonts::model::Source;
use read_fonts::types::{GlyphId, NameId, Tag};
use read_fonts::{FontRef, TableProvider};

use crate::font::Font;

/// Seconds from 1904-01-01 (the `head` table's epoch) to 1970-01-01.
const SECONDS_1904_TO_1970: i64 = 2_082_844_800;

/// Basic facts about a font, from its `name`, `head` and `OS/2` tables.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct FontInfo {
    /// Typographic family name if the font has one, otherwise the family name.
    pub family_name: Option<String>,
    /// Typographic subfamily name if the font has one, otherwise the
    /// subfamily name, e.g. "Bold Italic".
    pub subfamily_name: Option<String>,
    pub full_name: Option<String>,
    pub postscript_name: Option<String>,
    /// The version string, e.g. "Version 3.011".
    pub version: Option<String>,
    /// `head.fontRevision`, e.g. 3.011.
    pub font_revision: f32,
    /// Four-character vendor ID from `OS/2`, e.g. "GOOG", trailing spaces trimmed.
    pub vendor_id: Option<String>,
    /// `OS/2` weight class: 100 (thin) to 900 (black), 400 regular, 700 bold.
    pub weight_class: Option<u16>,
    /// `OS/2` width class: 1 (ultra-condensed) to 9 (ultra-expanded), 5 normal.
    pub width_class: Option<u16>,
    /// `OS/2.fsType` embedding permission bits; 0 = installable.
    pub embedding: Option<u16>,
    /// Creation and modification dates from `head`, in seconds since the Unix epoch.
    pub created: Option<i64>,
    pub modified: Option<i64>,
    pub num_glyphs: u32,
}

/// One localized string from the `name` table.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct NameEntry {
    /// Name ID: 0 copyright, 1 family, 2 subfamily, 4 full name, 5 version,
    /// 6 PostScript name, 9 designer, 13 license, 16/17 typographic family and
    /// subfamily, 256 and up for font-specific names (axes, instances, ...).
    pub id: u16,
    /// BCP 47 language tag, e.g. "en-US", or `None` if unknown.
    pub language: Option<String>,
    pub value: String,
}

/// A predefined location of a variable font, e.g. "Bold Condensed".
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct NamedInstance {
    /// Subfamily name, e.g. "Bold Condensed".
    pub name: Option<String>,
    pub postscript_name: Option<String>,
    /// The value of every axis; pass to `Font::set_variations`.
    pub variations: BTreeMap<String, f32>,
}

/// An entry in the font's table directory.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct FontTable {
    /// Four-character tag, e.g. "glyf", "CFF " (with its trailing space).
    pub tag: String,
    /// Size in bytes.
    pub length: u32,
}

impl Font {
    /// Names and basic facts from the `name`, `head` and `OS/2` tables.
    pub fn info(&self) -> FontInfo {
        let font = self.base();
        let tables = font.tables();
        let head = tables.head().ok();
        let os2 = tables.os2().ok();
        let first_name = |ids: &[NameId]| ids.iter().find_map(|&id| self.name(id.to_u16()));
        let unix = |secs: i64| secs - SECONDS_1904_TO_1970;
        FontInfo {
            family_name: first_name(&[NameId::TYPOGRAPHIC_FAMILY_NAME, NameId::FAMILY_NAME]),
            subfamily_name: first_name(&[
                NameId::TYPOGRAPHIC_SUBFAMILY_NAME,
                NameId::SUBFAMILY_NAME,
            ]),
            full_name: first_name(&[NameId::FULL_NAME]),
            postscript_name: first_name(&[NameId::POSTSCRIPT_NAME]),
            version: first_name(&[NameId::VERSION_STRING]),
            font_revision: head.as_ref().map_or(0.0, |h| h.font_revision().to_f32()),
            vendor_id: os2
                .as_ref()
                .map(|t| tag_string(t.ach_vend_id()).trim_end().to_string())
                .filter(|v| !v.is_empty()),
            weight_class: os2.as_ref().map(|t| t.us_weight_class()),
            width_class: os2.as_ref().map(|t| t.us_width_class()),
            embedding: os2.as_ref().map(|t| t.fs_type()),
            created: head.as_ref().map(|h| unix(h.created().as_secs())),
            modified: head.as_ref().map(|h| unix(h.modified().as_secs())),
            num_glyphs: font.num_glyphs(),
        }
    }

    /// A name from the `name` table by ID, in US English if available,
    /// otherwise English, otherwise the first one.
    pub fn name(&self, id: u16) -> Option<String> {
        let name = self.base().names().get(NameId::new(id))?;
        Some(name.english_or_first()?.name().to_string())
    }

    /// Every decodable string in the `name` table, by ID, in table order
    /// within each ID.
    pub fn names(&self) -> Vec<NameEntry> {
        let font = self.base();
        let ids: BTreeSet<u16> = font
            .tables()
            .name()
            .map(|t| {
                t.name_record()
                    .iter()
                    .map(|r| r.name_id().to_u16())
                    .collect()
            })
            .unwrap_or_default();
        let names = font.names();
        ids.into_iter()
            .filter_map(|id| names.get(NameId::new(id)))
            .flat_map(|name| {
                let id = name.id().to_u16();
                name.iter().map(move |l| NameEntry {
                    id,
                    language: l.language().map(|lang| lang.to_string()),
                    value: l.name().to_string(),
                })
            })
            .collect()
    }

    /// The glyph the font's character map gives `ch`, or `None` if it has none.
    pub fn glyph_id(&self, ch: char) -> Option<u16> {
        let id = self.base().charmap().map_unicode(ch)?;
        u16::try_from(id.to_u32()).ok()
    }

    /// Every character the font's character map covers, in code point order.
    pub fn characters(&self) -> Vec<char> {
        let chars: BTreeSet<char> = self
            .base()
            .charmap()
            .unicodes()
            .filter_map(|(cp, _)| char::from_u32(cp))
            .collect();
        chars.into_iter().collect()
    }

    /// The glyph's name from the `post` or `CFF` table, e.g. "a.ss01", or
    /// `None` if the font doesn't name it.
    pub fn glyph_name(&self, glyph_id: u16) -> Option<String> {
        let name = self.base().glyph_name(GlyphId::new(glyph_id.into()))?;
        (!name.is_synthesized()).then(|| name.to_string())
    }

    /// The glyph with this name, e.g. "a.ss01".
    pub fn glyph_by_name(&self, name: &str) -> Option<u16> {
        let id = self.base().glyph_by_name(name)?;
        u16::try_from(id.to_u32()).ok()
    }

    /// OpenType feature tags in the font's `GSUB` and `GPOS` tables, sorted,
    /// e.g. `["kern", "liga", "smcp", ...]`. Any of them can be turned on or off
    /// with `features`. Legacy `kern` tables and AAT (`morx`) features aren't listed.
    pub fn features(&self) -> Vec<String> {
        let tables = self.base().tables();
        let mut tags = BTreeSet::new();
        if let Ok(list) = tables.gsub().and_then(|t| t.feature_list()) {
            tags.extend(list.feature_records().iter().map(|r| r.feature_tag()));
        }
        if let Ok(list) = tables.gpos().and_then(|t| t.feature_list()) {
            tags.extend(list.feature_records().iter().map(|r| r.feature_tag()));
        }
        tags.into_iter().map(tag_string).collect()
    }

    /// OpenType script tags in the font's `GSUB` and `GPOS` tables, sorted,
    /// e.g. `["DFLT", "arab", "latn"]`.
    pub fn scripts(&self) -> Vec<String> {
        let tables = self.base().tables();
        let mut tags = BTreeSet::new();
        if let Ok(list) = tables.gsub().and_then(|t| t.script_list()) {
            tags.extend(list.script_records().iter().map(|r| r.script_tag()));
        }
        if let Ok(list) = tables.gpos().and_then(|t| t.script_list()) {
            tags.extend(list.script_records().iter().map(|r| r.script_tag()));
        }
        tags.into_iter().map(tag_string).collect()
    }

    /// The named instances of a variable font (empty for static fonts).
    pub fn named_instances(&self) -> Vec<NamedInstance> {
        self.base()
            .named_instances()
            .iter()
            .map(|i| NamedInstance {
                name: self.name(i.subfamily_name().to_u16()),
                postscript_name: i.postscript_name().and_then(|id| self.name(id.to_u16())),
                variations: i
                    .variations()
                    .map(|v| (tag_string(v.tag), v.value))
                    .collect(),
            })
            .collect()
    }

    /// The font's tables, in directory order.
    pub fn tables(&self) -> Vec<FontTable> {
        let Source::Blob(blob) = self.base().source() else {
            return Vec::new();
        };
        let Ok(font) = FontRef::from_index(blob, self.face_index()) else {
            return Vec::new();
        };
        font.table_directory()
            .table_records()
            .iter()
            .map(|r| FontTable {
                tag: tag_string(r.tag()),
                length: r.length(),
            })
            .collect()
    }
}

fn tag_string(tag: Tag) -> String {
    String::from_utf8_lossy(&tag.to_be_bytes()).into_owned()
}

#[cfg(test)]
mod tests {
    use crate::font::{load_test_font, test_font};

    #[test]
    fn info_and_names() {
        let font = test_font();
        let info = font.info();
        assert_eq!(info.family_name.as_deref(), Some("Roboto"));
        assert_eq!(info.subfamily_name.as_deref(), Some("Regular"));
        assert_eq!(info.weight_class, Some(400));
        assert_eq!(info.width_class, Some(5));
        assert!(info.num_glyphs > 100);
        // Some time after 2000-01-01.
        assert!(info.created.unwrap() > 946_684_800);

        assert_eq!(font.name(1).as_deref(), Some("Roboto"));
        assert_eq!(font.name(60000), None);
        let names = font.names();
        assert!(names.iter().any(|n| n.id == 1 && n.value == "Roboto"));
        assert!(names.windows(2).all(|w| w[0].id <= w[1].id), "sorted by id");
    }

    #[test]
    fn characters_and_glyphs() {
        let font = test_font();
        let a = font.glyph_id('a').expect("a is mapped");
        assert_eq!(
            font.shape("a", &Default::default()).unwrap().glyphs[0].id,
            a
        );
        assert_eq!(font.glyph_id('\u{10FFFF}'), None);
        let chars = font.characters();
        assert!(chars.contains(&'a') && chars.contains(&'é'));
        assert!(chars.windows(2).all(|w| w[0] < w[1]));

        // Roboto's post table (version 3) has no glyph names; Roboto Flex's does.
        assert_eq!(font.glyph_name(a), None, "not a synthesized gidNNN");
        let flex = load_test_font("RobotoFlex.ttf");
        let flex_a = flex.glyph_id('a').unwrap();
        assert_eq!(flex.glyph_name(flex_a).as_deref(), Some("uni0061"));
        assert_eq!(flex.glyph_by_name("uni0061"), Some(flex_a));
        assert_eq!(flex.glyph_by_name("no such glyph"), None);
        assert_eq!(flex.glyph_name(u16::MAX), None);
    }

    #[test]
    fn features_and_scripts() {
        let font = test_font();
        let features = font.features();
        for tag in ["kern", "liga", "smcp"] {
            assert!(features.iter().any(|f| f == tag), "{tag} in {features:?}");
        }
        assert!(features.windows(2).all(|w| w[0] < w[1]));
        assert!(font.scripts().iter().any(|s| s == "latn"));

        let arabic = load_test_font("NotoSansArabic.ttf");
        assert!(arabic.scripts().iter().any(|s| s == "arab"));
        assert!(arabic.features().iter().any(|f| f == "init"));
    }

    #[test]
    fn named_instances_of_a_variable_font() {
        assert!(test_font().named_instances().is_empty());
        let mut flex = load_test_font("RobotoFlex.ttf");
        let instances = flex.named_instances();
        let bold = instances
            .iter()
            .find(|i| i.name.as_deref() == Some("Bold"))
            .expect("a Bold instance");
        assert_eq!(bold.variations.get("wght"), Some(&700.0));
        assert_eq!(bold.variations.len(), flex.variation_axes().len());
        let axes: Vec<(&str, f32)> = bold
            .variations
            .iter()
            .map(|(k, v)| (k.as_str(), *v))
            .collect();
        flex.set_variations(&axes).unwrap();
    }

    #[test]
    fn table_directory() {
        let tables = test_font().tables();
        for tag in ["head", "glyf", "cmap", "name"] {
            assert!(tables.iter().any(|t| t.tag == tag && t.length > 0), "{tag}");
        }
    }
}
