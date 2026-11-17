# text-shape

Text shaping, metrics, Unicode segmentation and paragraph layout in pure Rust, built on
[HarfRust](https://crates.io/crates/harfrust) and [read-fonts](https://crates.io/crates/read-fonts).

It positions glyphs and hands you their outlines; rendering is up to you.

```toml
[dependencies]
text-shape = "0.1"
```

## Usage

```rust,no_run
use text_shape::{Font, FontStyle, HorizAlign, LayoutOptions, VertAlign};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut font = Font::from_bytes(std::fs::read("Roboto-Regular.ttf")?, 0)?;

    // Bold/italic: real axes on variable fonts, faux parameters otherwise
    let style = font.resolve_style(&FontStyle { bold: true, ..Default::default() });
    let axes: Vec<(&str, f32)> = style.variations.iter().map(|(k, v)| (k.as_str(), *v)).collect();
    font.set_variations(&axes)?;

    let text = "Hello world, wrapped in a box";
    let layout = font.layout(text, &LayoutOptions {
        x: 50.0,
        y: 50.0,
        size: 48.0,
        max_width: Some(400.0),
        align_h: HorizAlign::Center,
        align_v: VertAlign::Top,
        ..Default::default()
    })?;

    for line in &layout.lines {
        println!("{:?} at baseline {}", &text[line.start..line.end], line.baseline);
        for g in &line.glyphs {
            let _outline = font.outline(g.id); // font units, y up
            // point (px, py) maps to (g.x + px * layout.scale, g.y - py * layout.scale)
        }
    }

    println!("ink box: {:?}", layout.ink);
    Ok(())
}
```

### Kerning specific pairs

The font's kerning applies automatically. To adjust particular pairs, add entries to
`kerning`: each adds `px` (negative tightens) between the character at byte offset
`at` and the next one. To turn a feature off for part of the text, give it a range.

```rust,no_run
use text_shape::{Font, Kerning, LayoutOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let font = Font::from_bytes(std::fs::read("Roboto-Regular.ttf")?, 0)?;
    let layout = font.layout("AVATAR", &LayoutOptions {
        size: 48.0,
        kerning: vec![
            Kerning { at: 0, px: -4.0 }, // A–V 4px tighter
            Kerning { at: 3, px: 2.0 },  // T–A 2px looser
        ],
        features: vec!["kern[0:2]=0".into()], // no font kerning for "AV"
        ..Default::default()
    })?;
    println!("{}px wide", layout.width());
    Ok(())
}
```

## What's included

|                                         |                                                                                                                                                                                                                                                                                  |
| --------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Font`                                  | Load TTF/OTF/TTC; `metrics`, `variation_axes`, `set_variations`, `glyph_bbox`, `outline`, `resolve_style`                                                                                                                                                                        |
| `Font::shape`                           | Shape one run with direction, script, language and OpenType features                                                                                                                                                                                                             |
| `Font::layout`                          | Hard breaks, word or character wrapping, bidi reordering, alignment (`Start`/`End` follow the text direction), letter/word spacing, manual kerning of specific pairs (`LayoutOptions::kerning`), OpenType features on part of the text (`"kern[3:5]=0"`), loose and tight bounds |
| `line_breaks`, `graphemes`, `bidi_line` | Where lines may break ([UAX #14](https://www.unicode.org/reports/tr14/)), where each user-perceived character starts ([UAX #29](https://www.unicode.org/reports/tr29/)), and how mixed-direction text is ordered ([UAX #9](https://www.unicode.org/reports/tr9/))                |

Text indices (glyph clusters, line ranges, break positions) are UTF-8 byte offsets
into the input. Font values are in font units with y up; layout results are in px
with y down.

Enable the `serde` feature for `Serialize`/`Deserialize` on the data types.

The same engine powers the [`text-shape`](https://www.npmjs.com/package/text-shape)
npm package via WebAssembly.

## License

MIT
