# text-shape

[![npm](https://img.shields.io/npm/v/text-shape)](https://www.npmjs.com/package/text-shape)
[![CI](https://github.com/yyyk/text-shape/actions/workflows/ci.yml/badge.svg)](https://github.com/yyyk/text-shape/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

Text layout and measurement: shaping, metrics, wrapping, alignment and bidi text,
built on a Rust/WebAssembly core
([HarfRust](https://github.com/harfbuzz/harfrust) shaping, read-fonts metrics and
outlines, Unicode line breaking, bidi and grapheme segmentation).

It positions glyphs and hands you their outlines; drawing is up to you (Canvas 2D,
SVG, WebGL, or effects built on the outlines). Works in browsers and Node with no
bundler configuration: the WASM binary is embedded.

```sh
npm install text-shape
```

## Usage

```ts
import { loadFont, Typography, CENTER, TOP } from 'text-shape';

const font = await loadFont('/fonts/Roboto-Regular.ttf');
const t = new Typography().textFont(font, 48).textAlign(CENTER, TOP).textLeading(56);

// Lines and positioned glyphs, wrapped to a 400 × 300 box at (50, 50)
const layout = t.layout('Hello world, wrapped in a box', 50, 50, 400, 300);

t.textWidth('Hi'); // tight (ink) width, px
t.fontWidth('Hi'); // loose (advance) width, px
t.textBounds('Hi', 0, 0); // { x, y, w, h }
```

### Drawing with Canvas 2D

`font.path()` returns a cached `Path2D` per glyph, in font units with y pointing
down. Place it at the glyph's pen position and scale it to px:

```ts
import type { TextLayout } from 'text-shape';

function drawText(ctx: CanvasRenderingContext2D, layout: TextLayout) {
  const { font, scale, variations, synthetic } = layout;
  for (const line of layout.lines) {
    for (const g of line.glyphs) {
      if (!g.bbox) continue; // spaces and other empty glyphs
      const path = font.path(g.id, variations);
      ctx.save();
      ctx.translate(g.x, g.y);
      ctx.scale(scale, scale);
      // Faux italic/bold, only set when the font has no real italic/bold
      if (synthetic.oblique) ctx.transform(1, 0, -synthetic.oblique, 1, 0, 0);
      ctx.fill(path);
      if (synthetic.bold) {
        ctx.lineWidth = synthetic.bold; // font units
        ctx.lineJoin = 'round';
        ctx.strokeStyle = ctx.fillStyle;
        ctx.stroke(path);
      }
      ctx.restore();
    }
  }
}

ctx.fillStyle = 'black';
drawText(ctx, layout);
```

### Working with outlines

For effects built on glyph shapes, `font.outline()` returns the raw path commands:

```ts
const { lines, scale, font, variations } = t.layout('HELLO', 0, 0);
for (const g of lines.flatMap(l => l.glyphs)) {
  const outline = font.outline(g.id, variations); // font units, y up
  // each point (px, py) maps to canvas (g.x + px * scale, g.y - py * scale)
}
```

### Kerning specific pairs

The font's kerning applies automatically. To adjust particular pairs, pass
`kerning` to `layout()`: each entry adds `px` (negative tightens) between the
character at `at` and the next one. Positions are UTF-16 offsets, as everywhere in
this package.

```ts
const text = 'AVATAR';
t.layout(text, 0, 0, undefined, undefined, {
  kerning: [
    { at: 0, px: -4 }, // A–V 4px tighter
    { at: 3, px: 2 }, // T–A 2px looser
  ],
});
```

To turn kerning (or any OpenType feature) off for part of the text, give the
feature a range:

```ts
t.textProperty('fontFeatures', ['kern[0:2]=0']); // no font kerning for "AV"
```

## API

Setters are chainable; call them with no arguments to read the current value.

| Function                                                                | Description                                                                                                                                                                                                                             |
| ----------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `loadFont(url, faceIndex?)`, `Font.fromBytes(bytes)`                    | Load a TTF/OTF (or a face from a TTC)                                                                                                                                                                                                   |
| `new Typography()`                                                      | Holds text settings; measures and lays out text                                                                                                                                                                                         |
| `layout(str, x, y, maxWidth?, maxHeight?, extras?)`                     | Lines and positioned glyphs; wraps when `maxWidth` is given. `extras.kerning` adjusts specific character pairs                                                                                                                          |
| `font.path(glyphId, variations?)`, `font.outline(glyphId, variations?)` | Glyph shape as a cached `Path2D`, or as raw path commands                                                                                                                                                                               |
| `textFont(font, size?)`, `textSize(px)`, `textLeading(px)`              | Font, size and line spacing (leading defaults to 1.25 × size)                                                                                                                                                                           |
| `textAlign(h, v?)`                                                      | `START` (default) `END` `LEFT` `CENTER` `RIGHT` × `TOP` `CENTER` `BOTTOM` `BASELINE` (default). `START` and `END` follow the text direction, like CSS `text-align`: `START` is left for left-to-right text and right for right-to-left  |
| `textWrap(mode)`                                                        | `WORD` (break where the Unicode line breaking rules allow, [UAX #14](https://www.unicode.org/reports/tr14/)) or `CHAR` (between characters, keeping emoji and accented letters whole, [UAX #29](https://www.unicode.org/reports/tr29/)) |
| `textDirection(dir)`                                                    | `"ltr"`, `"rtl"` or `"inherit"` (detect from text)                                                                                                                                                                                      |
| `textWeight(n)`                                                         | `wght` axis of variable fonts; with `BOLD`, at least 700                                                                                                                                                                                |
| `textStyle(style)`                                                      | `NORMAL` `ITALIC` `BOLD` `BOLDITALIC`: font axes when available, otherwise `layout().synthetic` says how to fake it                                                                                                                     |
| `textProperty(name, value?)`, `textProperties(obj?)`                    | `fontKerning`, `fontVariantCaps`, `letterSpacing`, `wordSpacing`, `direction`, `fontFeatures`, `fontVariationSettings`                                                                                                                  |
| `fontAscent()`, `fontDescent()`                                         | Ascent/descent from the font's metrics                                                                                                                                                                                                  |
| `textAscent(str?)`, `textDescent(str?)`                                 | Tight ascent/descent of the first line of `str`, or the font's if omitted                                                                                                                                                               |
| `fontWidth(str)`, `fontBounds(...)`                                     | Loose (advance-based) width and box                                                                                                                                                                                                     |
| `textWidth(str)`, `textBounds(...)`                                     | Tight (ink) width and box                                                                                                                                                                                                               |

The measurements that take a string (`textAscent`, `textDescent`, `fontWidth`,
`textWidth`, `fontBounds`, `textBounds`) accept the same optional `extras` as
`layout()`, as their last argument, so kerned text measures as it's laid out:
`t.textWidth("AVATAR", { kerning })`.

Units are px with y down. Text indices (glyph `cluster`, line ranges) are UTF-16
offsets, so they work with `String.prototype.slice`.

`maxHeight` affects vertical alignment only (lines are not dropped). Faux bold and
italic are not included in bounds.

### Low-level

The raw WASM functions (`layout`, `shape`, `resolve_style`, `get_metrics`,
`get_outline`, `line_breaks`, `graphemes`, `bidi_line`, ...) are available from
`text-shape/wasm`. Call `initText()` first.

### Rust

The layout engine is a plain Rust crate, also published as
[`text-shape`](https://crates.io/crates/text-shape) on crates.io, with the same
features. The npm package is a WebAssembly build of it.

### Loading the WASM yourself

The embedded binary is about 420 KB gzipped. To serve the separate `.wasm` file
instead (about 330 KB gzipped, and compiled while it downloads), pass it to
`initText()` before loading fonts:

```ts
import { initText } from 'text-shape';
import wasmUrl from 'text-shape/text_shape_bg.wasm?url'; // Vite

await initText(wasmUrl);
```

In Node, `Path2D` is unavailable, so use `font.outline()` instead of `font.path()`.
Layout and metrics work as in the browser:

```ts
import { readFile } from 'node:fs/promises';
import { Font, initText, Typography } from 'text-shape';

await initText();
const font = Font.fromBytes(await readFile('Roboto-Regular.ttf'));
```

## Development

Requires Rust (via rustup) with the `wasm32-unknown-unknown` target, and `wasm-pack`.

This package lives in `js/` of the repository, next to the Rust crates it builds on:

- `crates/text-shape`: the layout engine, pure Rust
- `crates/text-shape-wasm`: WebAssembly bindings used by this package
- `js/src`: the TypeScript API (`Typography`, `Font` caching, `initText`)

```sh
npm run build      # wasm-pack + embed + tsc
npm test           # build, then the JS tests in test/
npm run test:rust  # Rust tests and doctests of the whole workspace
npm run dev        # tsc --watch
```

## License

MIT
