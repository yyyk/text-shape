//! Font introspection: names, character coverage, glyph names, OpenType
//! features, named instances and the table directory.

use wasm_bindgen::prelude::*;

use crate::font::with_font;
use crate::util::to_js;

/// Names and basic facts from the `name`, `head` and `OS/2` tables.
#[wasm_bindgen(unchecked_return_type = "FontInfo")]
pub fn get_font_info(font_id: u32) -> Result<JsValue, JsError> {
    to_js(&with_font(font_id, |f| f.info())?)
}

/// A name from the `name` table by ID, in US English if available, otherwise
/// English, otherwise the first one.
#[wasm_bindgen]
pub fn get_name(font_id: u32, name_id: u16) -> Result<Option<String>, JsError> {
    with_font(font_id, |f| f.name(name_id))
}

/// Every decodable string in the `name` table, by ID.
#[wasm_bindgen(unchecked_return_type = "NameEntry[]")]
pub fn get_names(font_id: u32) -> Result<JsValue, JsError> {
    to_js(&with_font(font_id, |f| f.names())?)
}

/// The glyph the character map gives a code point, or undefined.
#[wasm_bindgen]
pub fn get_glyph_id(font_id: u32, code_point: u32) -> Result<Option<u16>, JsError> {
    let Some(ch) = char::from_u32(code_point) else {
        return Ok(None);
    };
    with_font(font_id, |f| f.glyph_id(ch))
}

/// Every code point the character map covers, in order.
#[wasm_bindgen]
pub fn get_characters(font_id: u32) -> Result<Vec<u32>, JsError> {
    with_font(font_id, |f| {
        f.characters().into_iter().map(u32::from).collect()
    })
}

/// The glyph's name from the `post` or `CFF` table, or undefined.
#[wasm_bindgen]
pub fn get_glyph_name(font_id: u32, glyph_id: u16) -> Result<Option<String>, JsError> {
    with_font(font_id, |f| f.glyph_name(glyph_id))
}

/// The glyph with this name, or undefined.
#[wasm_bindgen]
pub fn get_glyph_by_name(font_id: u32, name: &str) -> Result<Option<u16>, JsError> {
    with_font(font_id, |f| f.glyph_by_name(name))
}

/// OpenType feature tags in `GSUB` and `GPOS`, sorted.
#[wasm_bindgen]
pub fn get_features(font_id: u32) -> Result<Vec<String>, JsError> {
    with_font(font_id, |f| f.features())
}

/// OpenType script tags in `GSUB` and `GPOS`, sorted.
#[wasm_bindgen]
pub fn get_scripts(font_id: u32) -> Result<Vec<String>, JsError> {
    with_font(font_id, |f| f.scripts())
}

/// The named instances of a variable font (empty for static fonts).
#[wasm_bindgen(unchecked_return_type = "NamedInstance[]")]
pub fn get_named_instances(font_id: u32) -> Result<JsValue, JsError> {
    to_js(&with_font(font_id, |f| f.named_instances())?)
}

/// The font's tables, in directory order.
#[wasm_bindgen(unchecked_return_type = "FontTable[]")]
pub fn get_tables(font_id: u32) -> Result<JsValue, JsError> {
    to_js(&with_font(font_id, |f| f.tables())?)
}
