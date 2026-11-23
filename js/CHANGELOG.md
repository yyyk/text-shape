# Changelog

All notable changes to the `text-shape` npm package are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/) (while the version is 0.x, minor
versions may change the API).

The Rust crate is versioned separately; see
[its changelog](https://github.com/yyyk/text-shape/blob/main/crates/text-shape/CHANGELOG.md).

## [0.1.5] - 2026-11-23

### Added

- Font introspection: `font.info()` (names, version, vendor, weight and width
  class, embedding bits, dates, glyph count), `name()` and `names()` (the `name`
  table, with languages), `glyphId()` and `characters()` (the character map),
  `glyphName()` and `glyphByName()`, `features()` and `scripts()` (from `GSUB`
  and `GPOS`), `namedInstances()` of variable fonts, and `tables()` (the table
  directory). The raw functions (`get_font_info`, `get_names`, ...) are in
  `text-shape/wasm`.

### Changed

- A non-zero `letterSpacing` turns off optional ligatures (`liga`, `clig`,
  `dlig`, `hlig`), as in CSS, so spaced-out text no longer keeps "fi" squeezed
  together. Turn one back on with `fontFeatures`, e.g. `['liga']`.
- The WASM binary is about 7% larger (30 KB gzipped) for the introspection
  functions.

### Fixed

- Wrapped lines no longer come out wider than `maxWidth` when shaping changes at
  the break: Arabic letters taking their wider final form at a line end (by up
  to 33px at 48px with `CHAR` wrapping), or kerning between the letters either
  side of a `CHAR` break.

## [0.1.4] - 2026-11-18

### Changed

- `font.outline()` returns `readonly Readonly<PathCommand>[]`: the array is
  cached, so changing it in place would change every later outline of that glyph.

## [0.1.3] - 2026-11-18

### Changed

- `font.outline()` returns `PathCommand`s, `{ type: 'moveTo', x, y }` and so on,
  the same shape stroke-outline and vector-path-tools use, instead of `PathCmd`s
  like `{ MoveTo: [x, y] }`. `get_outline` returns the same.

### Added

- `font.outline(glyphId, variations, { flipY: true })` returns the outline y down.

## [0.1.2] - 2026-11-17

### Changed

- Shaping now uses HarfRust, and fonts are read with read-fonts, instead of
  rustybuzz and ttf-parser, which are no longer developed. The API is unchanged.
- Outlines no longer end each contour with a line back to its start before
  `Close`; `Close` draws that line, so the shape is the same.
- In variable fonts, outlines and glyph bounds at axis values whose variations
  have fractions are rounded to whole font units, as FreeType does, instead of
  keeping half units: they move by at most half a unit.

### Fixed

- Kerning in variable fonts away from their default axis values now follows the
  font's kerning variations, matching HarfBuzz; it was off by up to a few
  hundred font units per line (e.g. "AV", "To" in Roboto Flex at `wght` 100).
- `hidden` on variation axes now reads the font's hidden flag; it read the wrong
  bit, so axes such as Roboto Flex's parametric ones (`XOPQ`, `YTUC`, ...) were
  reported as not hidden.

## [0.1.1] - 2026-11-16

### Added

- `text-shape/package.json` is exported, for tools that read the package's
  metadata.

## [0.1.0] - 2026-11-16

First release.

### Added

- `Font`: load TTF, OTF and TTC fonts; metrics, variation axes, glyph bounds and
  outlines. Variable fonts with many axes (e.g. Roboto Flex) are supported.
- Shaping with direction, script, language and OpenType features (kerning,
  ligatures, small caps, ...).
- Paragraph layout: hard line breaks; wrapping by word, following the Unicode
  line breaking rules, or by character, without splitting emoji or accented
  letters; mixed left-to-right and right-to-left text, ordered by the Unicode
  bidirectional algorithm; horizontal and vertical alignment, leading, letter and
  word spacing. Horizontal alignment defaults to `start`, which, as in CSS and
  Canvas, is the left edge for left-to-right text and the right edge for
  right-to-left text.
- Manual kerning: extra space (or less) between specific character pairs, on top
  of the font's kerning, in both writing directions. The measurement functions
  accept it too, so kerned text measures as it's laid out.
- OpenType features on part of the text, using HarfBuzz's range syntax, e.g.
  `kern[3:5]=0` to turn kerning off for two characters.
- Loose (advance) and tight (ink) widths and bounds.
- Weight and style: real `wght`, `ital` and `slnt` axes when the font has them,
  otherwise faux bold and italic parameters for drawing. Bold means a weight of
  at least 700, so it shows even when a lighter weight is set.
- Unicode segmentation on its own: where lines may break, where each
  user-perceived character starts, and how mixed-direction text is ordered.
- npm package: a `Typography` API for settings, measurement and layout, with
  `Font` caching of outlines and `Path2D`s. Works in browsers and Node 20+ with
  no bundler setup: the WebAssembly is embedded. Text indices are UTF-16
  offsets, so they work with `String.prototype.slice`.

[0.1.5]: https://github.com/yyyk/text-shape/releases/tag/v0.1.5
[0.1.4]: https://github.com/yyyk/text-shape/releases/tag/v0.1.4
[0.1.3]: https://github.com/yyyk/text-shape/releases/tag/v0.1.3
[0.1.2]: https://github.com/yyyk/text-shape/releases/tag/v0.1.2
[0.1.1]: https://github.com/yyyk/text-shape/releases/tag/v0.1.1
[0.1.0]: https://github.com/yyyk/text-shape/releases/tag/v0.1.0
