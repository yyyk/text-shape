# Changelog

All notable changes to the `text-shape` Rust crate are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/) (while the version is 0.x, minor
versions may change the API).

The npm package is versioned separately; see
[its changelog](https://github.com/yyyk/text-shape/blob/main/js/CHANGELOG.md).

## [Unreleased]

### Added

- Font introspection: `Font::info` (names, version, vendor, weight and width
  class, embedding bits, dates, glyph count), `name` and `names` (the `name`
  table, with languages), `glyph_id` and `characters` (the character map),
  `glyph_name` and `glyph_by_name`, `features` and `scripts` (from `GSUB` and
  `GPOS`), `named_instances` of variable fonts, and `tables` (the table
  directory).

### Changed

- Non-zero `letter_spacing` turns off optional ligatures (`liga`, `clig`,
  `dlig`, `hlig`), as in CSS, so spaced-out text no longer keeps "fi" squeezed
  together. Turn one back on through `features`, e.g. `"liga"`.

- Shaping now uses HarfRust, and fonts are read with read-fonts, instead of
  rustybuzz and ttf-parser, which are no longer developed. The API is unchanged.
- Outlines no longer end each contour with a line back to its start before
  `Close`; `Close` draws that line, so the shape is the same.
- In variable fonts, outlines and glyph bounds at axis values whose variations
  have fractions are rounded to whole font units, as FreeType does, instead of
  keeping half units: they move by at most half a unit.

### Fixed

- Wrapped lines no longer come out wider than `max_width` when shaping changes
  at the break: Arabic letters taking their wider final form at a line end
  (by up to 33px at 48px with character wrapping), or kerning between the
  letters either side of a character-wrap break.
- Kerning in variable fonts away from their default axis values now follows the
  font's kerning variations, matching HarfBuzz; it was off by up to a few
  hundred font units per line (e.g. "AV", "To" in Roboto Flex at `wght` 100).
- `hidden` on variation axes now reads the font's hidden flag; it read the wrong
  bit, so axes such as Roboto Flex's parametric ones (`XOPQ`, `YTUC`, ...) were
  reported as not hidden.

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
- Rust crate: the same engine as plain Rust (Rust 1.85+), with text indices as
  UTF-8 byte offsets and an optional `serde` feature.
