use text_shape::BaseDirection;
use wasm_bindgen::prelude::*;

use crate::util::{js_err, to_js, utf8_to_utf16_table, utf16_to_utf8};

/// Line break opportunities (UAX #14).
#[wasm_bindgen(unchecked_return_type = "LineBreak[]")]
pub fn line_breaks(text: String) -> Result<JsValue, JsError> {
    let to_utf16 = utf8_to_utf16_table(&text);
    let mut breaks = text_shape::line_breaks(&text);
    for b in &mut breaks {
        b.index = to_utf16[b.index];
    }
    to_js(&breaks)
}

/// Grapheme cluster boundaries (UAX #29), including 0 and the text length.
#[wasm_bindgen]
pub fn graphemes(text: String) -> Vec<u32> {
    let to_utf16 = utf8_to_utf16_table(&text);
    text_shape::graphemes(&text)
        .into_iter()
        .map(|i| to_utf16[i] as u32)
        .collect()
}

/// Bidi analysis (UAX #9) of one line of `text`, as runs in visual order. Pass
/// the whole paragraph plus the line's UTF-16 range so embedding levels are
/// resolved with full context.
#[wasm_bindgen(unchecked_return_type = "BidiLine")]
pub fn bidi_line(
    text: String,
    #[wasm_bindgen(unchecked_param_type = "\"ltr\" | \"rtl\" | \"auto\" | null | undefined")] base: Option<String>,
    line_start: Option<u32>,
    line_end: Option<u32>,
) -> Result<JsValue, JsError> {
    let base = match base.as_deref() {
        None | Some("auto") => BaseDirection::Auto,
        Some("ltr") => BaseDirection::Ltr,
        Some("rtl") => BaseDirection::Rtl,
        Some(other) => {
            return Err(js_err(format!(
                "invalid base direction {other:?}; expected ltr, rtl or auto"
            )));
        }
    };
    let start = line_start.map_or(0, |i| utf16_to_utf8(&text, i as usize));
    let end = line_end
        .map_or(text.len(), |i| utf16_to_utf8(&text, i as usize))
        .max(start);

    let to_utf16 = utf8_to_utf16_table(&text);
    let mut line = text_shape::bidi_line(&text, base, start..end);
    for run in &mut line.runs {
        run.start = to_utf16[run.start];
        run.end = to_utf16[run.end];
    }
    to_js(&line)
}
