//! Helpers for crossing the JS boundary.

use serde::Serialize;
use serde::de::DeserializeOwned;
use wasm_bindgen::prelude::*;

/// Serializes to a JS value: `None` → `null`, maps → plain objects.
pub fn to_js<T: Serialize>(value: &T) -> Result<JsValue, JsError> {
    let serializer = serde_wasm_bindgen::Serializer::new()
        .serialize_missing_as_null(true)
        .serialize_maps_as_objects(true);
    value
        .serialize(&serializer)
        .map_err(|e| JsError::new(&e.to_string()))
}

/// Deserializes an optional options object; `undefined`/`null` → `T::default()`.
pub fn from_js_or_default<T: DeserializeOwned + Default>(
    value: Option<JsValue>,
) -> Result<T, JsError> {
    match value {
        Some(v) if !v.is_null() && !v.is_undefined() => {
            serde_wasm_bindgen::from_value(v).map_err(|e| JsError::new(&e.to_string()))
        }
        _ => Ok(T::default()),
    }
}

pub fn js_err(e: impl std::fmt::Display) -> JsError {
    JsError::new(&e.to_string())
}

/// Maps every UTF-8 byte offset of `text` (0..=len) to its UTF-16 offset.
/// Offsets inside a multi-byte char map to that char's start.
pub fn utf8_to_utf16_table(text: &str) -> Vec<usize> {
    let mut table = vec![0; text.len() + 1];
    let mut u16_pos = 0;
    for (byte_pos, ch) in text.char_indices() {
        for slot in &mut table[byte_pos..byte_pos + ch.len_utf8()] {
            *slot = u16_pos;
        }
        u16_pos += ch.len_utf16();
    }
    table[text.len()] = u16_pos;
    table
}

/// Converts a UTF-16 offset to a UTF-8 byte offset, clamped to the text and
/// snapped forward to a char boundary.
pub fn utf16_to_utf8(text: &str, u16_index: usize) -> usize {
    let mut u16_pos = 0;
    for (byte_pos, ch) in text.char_indices() {
        if u16_pos >= u16_index {
            return byte_pos;
        }
        u16_pos += ch.len_utf16();
    }
    text.len()
}

/// Converts the range in a HarfBuzz feature string (`kern[3:5]=0`, `smcp[2]`,
/// `liga[:4]`, `kern[3:]`) from UTF-16 offsets into `text` to byte offsets.
/// Strings without a range are returned unchanged.
pub fn feature_to_utf8(feature: &str, text: &str) -> String {
    let (Some(open), Some(close)) = (feature.find('['), feature.find(']')) else {
        return feature.to_string();
    };
    if close < open {
        return feature.to_string();
    }
    let convert = |n: &str| match n.trim().parse::<usize>() {
        Ok(i) => utf16_to_utf8(text, i).to_string(),
        Err(_) => n.to_string(), // empty bound, or invalid: leave it to the parser
    };
    let range = &feature[open + 1..close];
    let converted = match range.split_once(':') {
        Some((a, b)) => format!("{}:{}", convert(a), convert(b)),
        None => convert(range),
    };
    format!("{}[{converted}]{}", &feature[..open], &feature[close + 1..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_table_handles_multibyte_and_astral() {
        // "a" (1 byte, 1 unit), "é" (2 bytes, 1 unit), "😀" (4 bytes, 2 units)
        let t = utf8_to_utf16_table("aé😀b");
        assert_eq!(t, vec![0, 1, 1, 2, 2, 2, 2, 4, 5]);
        assert_eq!(utf16_to_utf8("aé😀b", 2), 3);
        assert_eq!(utf16_to_utf8("aé😀b", 4), 7);
        assert_eq!(utf16_to_utf8("aé😀b", 99), 8);
    }

    #[test]
    fn feature_ranges_convert_to_bytes() {
        let text = "😀AV"; // A is UTF-16 index 2, byte 4
        assert_eq!(feature_to_utf8("kern[2:4]=0", text), "kern[4:6]=0");
        assert_eq!(feature_to_utf8("smcp[2]", text), "smcp[4]");
        assert_eq!(feature_to_utf8("kern[2:]=0", text), "kern[4:]=0");
        assert_eq!(feature_to_utf8("liga[:2]=0", text), "liga[:4]=0");
        assert_eq!(feature_to_utf8("kern=0", text), "kern=0");
        assert_eq!(feature_to_utf8("-liga", text), "-liga");
    }
}
