use text_shape::ShapeOptions;
use wasm_bindgen::prelude::*;

use crate::font::with_font;
use crate::util::{feature_to_utf8, from_js_or_default, js_err, to_js, utf8_to_utf16_table};

/// Shapes a single run of text. For mixed-direction text use `layout`, or split
/// it with `bidi_line` first and shape each run with its direction.
#[wasm_bindgen(unchecked_return_type = "ShapingResult")]
pub fn shape(
    font_id: u32,
    text: String,
    #[wasm_bindgen(unchecked_param_type = "ShapeOptions | null | undefined")] options: Option<
        JsValue,
    >,
) -> Result<JsValue, JsError> {
    let mut options: ShapeOptions = from_js_or_default(options)?;
    for f in &mut options.features {
        *f = feature_to_utf8(f, &text);
    }
    let mut result = with_font(font_id, |f| f.shape(&text, &options))?.map_err(js_err)?;
    let to_utf16 = utf8_to_utf16_table(&text);
    for g in &mut result.glyphs {
        g.cluster = to_utf16[g.cluster];
    }
    to_js(&result)
}
