//! Font registry, metrics, variations, style resolution and outlines.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use text_shape::{Font, FontStyle};
use wasm_bindgen::prelude::*;

use crate::util::{from_js_or_default, js_err, to_js};

thread_local! {
    static FONTS: RefCell<HashMap<u32, Font>> = RefCell::new(HashMap::new());
    static NEXT_ID: Cell<u32> = const { Cell::new(1) };
}

pub fn with_font<R>(id: u32, f: impl FnOnce(&mut Font) -> R) -> Result<R, JsError> {
    FONTS.with(|m| {
        let mut fonts = m.borrow_mut();
        let font = fonts
            .get_mut(&id)
            .ok_or_else(|| JsError::new(&format!("unknown font id {id}")))?;
        Ok(f(font))
    })
}

/// Loads a font (TTF/OTF, or a face from a TTC via `face_index`) and returns its id.
#[wasm_bindgen]
pub fn load_font(data: js_sys::Uint8Array, face_index: Option<u32>) -> Result<u32, JsError> {
    let font = Font::from_bytes(data.to_vec(), face_index.unwrap_or(0)).map_err(js_err)?;
    let id = NEXT_ID.with(|n| {
        let id = n.get();
        n.set(id + 1);
        id
    });
    FONTS.with(|m| m.borrow_mut().insert(id, font));
    Ok(id)
}

/// Frees a loaded font. Returns false if the id was unknown.
#[wasm_bindgen]
pub fn unload_font(font_id: u32) -> bool {
    FONTS.with(|m| m.borrow_mut().remove(&font_id).is_some())
}

/// Vertical metrics and basic info, in font units. Reflects current variations.
#[wasm_bindgen(unchecked_return_type = "FontMetrics")]
pub fn get_metrics(font_id: u32) -> Result<JsValue, JsError> {
    to_js(&with_font(font_id, |f| f.metrics())?)
}

/// Variation axes of a variable font (empty for static fonts).
#[wasm_bindgen(unchecked_return_type = "VariationAxis[]")]
pub fn get_variation_axes(font_id: u32) -> Result<JsValue, JsError> {
    to_js(&with_font(font_id, |f| f.variation_axes())?)
}

/// Sets axis values, e.g. `{ wght: 700, slnt: -10 }`, clamped to each axis range.
/// Unlisted axes keep their current value. Affects later `shape`, `layout`,
/// `get_outline` and `get_metrics` calls.
#[wasm_bindgen]
pub fn set_variations(
    font_id: u32,
    #[wasm_bindgen(unchecked_param_type = "Record<string, number>")] values: JsValue,
) -> Result<(), JsError> {
    let values: HashMap<String, f32> = serde_wasm_bindgen::from_value(values).map_err(js_err)?;
    let pairs: Vec<(&str, f32)> = values.iter().map(|(k, &v)| (k.as_str(), v)).collect();
    with_font(font_id, |f| f.set_variations(&pairs))?.map_err(js_err)
}

/// Resets every axis to its default value.
#[wasm_bindgen]
pub fn reset_variations(font_id: u32) -> Result<(), JsError> {
    with_font(font_id, |f| f.reset_variations())
}

/// Maps a requested weight/style onto the font: real axes when it has them,
/// otherwise faux bold/italic parameters.
#[wasm_bindgen(unchecked_return_type = "ResolvedStyle")]
pub fn resolve_style(
    font_id: u32,
    #[wasm_bindgen(unchecked_param_type = "FontStyle | null | undefined")] style: Option<JsValue>,
) -> Result<JsValue, JsError> {
    let style: FontStyle = from_js_or_default(style)?;
    to_js(&with_font(font_id, |f| f.resolve_style(&style))?)
}

/// Glyph outline in font units (y up). Reflects current variations.
#[wasm_bindgen(unchecked_return_type = "PathCmd[]")]
pub fn get_outline(font_id: u32, glyph_id: u16) -> Result<JsValue, JsError> {
    to_js(&with_font(font_id, |f| f.outline(glyph_id))?)
}
