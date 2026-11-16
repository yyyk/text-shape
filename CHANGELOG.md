# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/) (while the version is 0.x, minor
versions may change the API).

The Rust crate (`text-shape` on crates.io) and the npm package (`text-shape` on
npm) are released together with the same version number.

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
- Rust crate: the same engine as plain Rust (Rust 1.85+), with text indices as
  UTF-8 byte offsets and an optional `serde` feature.
