use text_shape::LayoutOptions;
use wasm_bindgen::prelude::*;

use crate::font::with_font;
use crate::util::{
    feature_to_utf8, from_js_or_default, js_err, to_js, utf8_to_utf16_table, utf16_to_utf8,
};

/// Lays out `text`: hard breaks, wrapping (with `max_width`), bidi reordering,
/// alignment and bounds. Uses the font's current variations.
#[wasm_bindgen(unchecked_return_type = "LayoutResult")]
pub fn layout(
    font_id: u32,
    text: String,
    #[wasm_bindgen(unchecked_param_type = "LayoutOptions | null | undefined")] options: Option<
        JsValue,
    >,
) -> Result<JsValue, JsError> {
    let mut options: LayoutOptions = from_js_or_default(options)?;
    for k in &mut options.kerning {
        k.at = utf16_to_utf8(&text, k.at);
    }
    for f in &mut options.features {
        *f = feature_to_utf8(f, &text);
    }
    let mut result = with_font(font_id, |f| f.layout(&text, &options))?.map_err(js_err)?;
    let to_utf16 = utf8_to_utf16_table(&text);
    for line in &mut result.lines {
        line.start = to_utf16[line.start];
        line.end = to_utf16[line.end];
        for g in &mut line.glyphs {
            g.cluster = to_utf16[g.cluster];
        }
    }
    to_js(&result)
}
