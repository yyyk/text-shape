//! WebAssembly bindings for the `text-shape` crate.
//!
//! Fonts live in a registry and are referred to by numeric id. All text indices
//! crossing the boundary (clusters, break positions, ranges) are converted from
//! UTF-8 byte offsets to UTF-16 code unit offsets, so JS can use them directly
//! with `String.slice`.

mod font;
mod layout;
mod segmentation;
mod shaping;
mod util;

use wasm_bindgen::prelude::*;

#[wasm_bindgen(typescript_custom_section)]
const TS_TYPES: &'static str = r#"
export type TextDirection = "ltr" | "rtl" | "ttb" | "btt";

export interface GlyphBBox {
  x_min: number;
  y_min: number;
  x_max: number;
  y_max: number;
}

export interface FontMetrics {
  units_per_em: number;
  ascender: number;
  descender: number;
  line_gap: number;
  cap_height: number | null;
  x_height: number | null;
  underline_position: number | null;
  underline_thickness: number | null;
  italic_angle: number;
  weight: number;
  is_italic: boolean;
  is_bold: boolean;
  is_monospaced: boolean;
  is_variable: boolean;
  family_name: string | null;
  global_bbox: GlyphBBox;
}

export interface VariationAxis {
  tag: string;
  min: number;
  default: number;
  max: number;
  hidden: boolean;
}

export interface FontStyle {
  /** `wght` value for variable fonts; defaults to 400. With `bold`, the weight is at least 700. */
  weight?: number | null;
  bold?: boolean;
  italic?: boolean;
}

/** Faux styling to apply when drawing, for fonts without a real bold/italic. */
export interface Synthetic {
  /** Stroke width in font units to embolden with (stroke the glyph path); 0 = none. */
  bold: number;
  /** Horizontal skew for a faux italic: shift x right by `oblique` × height above the baseline; 0 = none. */
  oblique: number;
}

export interface ResolvedStyle {
  /** Axis values to pass to `set_variations`. */
  variations: Record<string, number>;
  synthetic: Synthetic;
}

export interface ShapeOptions {
  /** Defaults to a guess from the text. */
  direction?: TextDirection;
  /** ISO 15924 tag, e.g. "Latn", "Arab". Defaults to a guess from the text. */
  script?: string;
  /** BCP 47 tag, e.g. "en", "tr". */
  language?: string;
  /**
   * HarfBuzz feature syntax, e.g. ["kern=0", "liga=0", "smcp", "ss01"]. A range
   * applies a feature to part of the text: "kern[3:5]=0" (UTF-16 offsets).
   */
  features?: string[];
}

export interface ShapedGlyph {
  id: number;
  /** UTF-16 offset of the first character this glyph came from. */
  cluster: number;
  /** Font units. */
  x_advance: number;
  y_advance: number;
  x_offset: number;
  y_offset: number;
  /** Ink bounds in font units (y up), or null for empty glyphs such as spaces. */
  bbox: GlyphBBox | null;
}

export interface ShapingResult {
  units_per_em: number;
  /** Resolved direction. Glyphs are always in visual (left-to-right) order. */
  direction: TextDirection;
  glyphs: ShapedGlyph[];
}

export type PathCmd =
  | { MoveTo: [number, number] }
  | { LineTo: [number, number] }
  | { QuadTo: [number, number, number, number] }
  | { CubicTo: [number, number, number, number, number, number] }
  | "Close";

export interface LineBreak {
  /** UTF-16 offset; a line may end right before this index. */
  index: number;
  /** Hard break (newline etc.). The final entry is always a mandatory break at text end. */
  mandatory: boolean;
}

export interface BidiRun {
  /** UTF-16 range [start, end). */
  start: number;
  end: number;
  level: number;
  direction: "ltr" | "rtl";
}

export interface BidiLine {
  base_direction: "ltr" | "rtl";
  /** In visual order (left to right). Shape each run with its direction. */
  runs: BidiRun[];
}

/** Layout settings. Units are px; every field is optional. */
export interface LayoutOptions {
  /** Anchor point; its meaning depends on the alignment. */
  x?: number;
  y?: number;
  /** Default 12. */
  size?: number;
  /** Distance between baselines. Defaults to 1.25 × size. */
  leading?: number | null;
  /** Added after every cluster. */
  letter_spacing?: number;
  /** Added after every space. */
  word_spacing?: number;
  /** Extra space between specific pairs of characters. */
  kerning?: Kerning[];
  /**
   * HarfBuzz feature syntax, e.g. ["kern=0", "smcp"]. A range applies a feature
   * to part of the text: "kern[3:5]=0" (UTF-16 offsets).
   */
  features?: string[];
  direction?: "ltr" | "rtl" | "auto";
  /** Only used when max_width is set. */
  wrap?: "word" | "char";
  /** "start" (the default) and "end" follow each paragraph's direction. */
  align_h?: "start" | "end" | "left" | "center" | "right";
  align_v?: "top" | "center" | "bottom" | "baseline";
  /** Wrap lines to this width, and align horizontally within it. */
  max_width?: number | null;
  /** Align vertically within this height. Text is not clipped. */
  max_height?: number | null;
}

/**
 * Extra space between the character at `at` and the next one, on top of the
 * font's kerning and letter spacing. For right-to-left text the next character
 * is the next in reading order, so the space appears on its left. Ignored for
 * the last character of a line or direction run; inside a ligature it applies
 * after the whole ligature.
 */
export interface Kerning {
  /** UTF-16 offset of the first character of the pair. */
  at: number;
  /** px; negative values tighten. */
  px: number;
}

/** A rectangle in px (y down). */
export interface Bounds {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface PositionedGlyph {
  id: number;
  /** UTF-16 offset into the input text. */
  cluster: number;
  /** Pen position in px at the baseline (y down). */
  x: number;
  y: number;
  /** Ink bounds in font units (y up), or null for empty glyphs. */
  bbox: GlyphBBox | null;
}

export interface LayoutLine {
  /** UTF-16 range of the line's visible text (trailing whitespace excluded). */
  start: number;
  end: number;
  /** Left edge of the line's advance box, px. */
  x: number;
  baseline: number;
  /** Advance width of the line, px. */
  width: number;
  /** Base direction of the line's paragraph. */
  direction: "ltr" | "rtl";
  glyphs: PositionedGlyph[];
  /** Tight box around the line's ink, or null if it has none. */
  ink: Bounds | null;
}

export interface LayoutResult {
  /** Always at least one line. */
  lines: LayoutLine[];
  /** px per font unit */
  scale: number;
  /** Font ascent and descent in px, both positive. */
  ascent: number;
  descent: number;
  /** Loose box: each line's advance width × font ascent/descent. */
  advance: Bounds;
  /** Tight box around all ink, or null if there is none. */
  ink: Bounds | null;
}
"#;
