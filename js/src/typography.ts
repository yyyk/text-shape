import { Font, type Variations } from './font.js';
import type {
	Bounds,
	Kerning,
	LayoutOptions,
	LayoutResult,
	Synthetic,
} from '../wasm/text_shape.js';

/** Alignment that follows the text direction: left for left-to-right, right for right-to-left. */
export const START = 'start';
/** The opposite of START: right for left-to-right text, left for right-to-left. */
export const END = 'end';
export const LEFT = 'left';
export const CENTER = 'center';
export const RIGHT = 'right';
export const TOP = 'top';
export const BOTTOM = 'bottom';
export const BASELINE = 'baseline';
export const WORD = 'word';
export const CHAR = 'char';
export const NORMAL = 'normal';
export const ITALIC = 'italic';
export const BOLD = 'bold';
export const BOLDITALIC = 'bolditalic';

export type HorizAlign = NonNullable<LayoutOptions['align_h']>;
export type VertAlign = NonNullable<LayoutOptions['align_v']>;
export type WrapMode = NonNullable<LayoutOptions['wrap']>;
export type TextStyle = typeof NORMAL | typeof ITALIC | typeof BOLD | typeof BOLDITALIC;
export type TextDirectionSetting = 'ltr' | 'rtl' | 'inherit';

/** A layout plus what's needed to draw it. */
export interface TextLayout extends LayoutResult {
	font: Font;
	/** Axis values the glyphs were shaped with; pass them to `font.outline()` / `font.path()`. */
	variations: Variations;
	synthetic: Synthetic;
}

/**
 * Text properties, named after the matching canvas context properties, plus
 * two extensions for direct font control.
 */
export interface TextProperties {
	fontKerning: 'auto' | 'normal' | 'none';
	fontVariantCaps:
		| 'normal'
		| 'small-caps'
		| 'all-small-caps'
		| 'petite-caps'
		| 'all-petite-caps'
		| 'unicase'
		| 'titling-caps';
	/** px, or a CSS length string in px such as "2px" */
	letterSpacing: number | string;
	wordSpacing: number | string;
	direction: TextDirectionSetting;
	/**
	 * Extension: raw OpenType features in HarfBuzz syntax, e.g. ["ss01", "tnum"].
	 * A range applies one to part of the text: "kern[3:5]=0" (UTF-16 offsets).
	 */
	fontFeatures: string[];
	/** Extension: raw axis values, e.g. { wdth: 75 }. Applied after weight/style. */
	fontVariationSettings: Variations;
}

const CAPS_FEATURES: Record<TextProperties['fontVariantCaps'], string[]> = {
	normal: [],
	'small-caps': ['smcp'],
	'all-small-caps': ['smcp', 'c2sc'],
	'petite-caps': ['pcap'],
	'all-petite-caps': ['pcap', 'c2pc'],
	unicase: ['unic'],
	'titling-caps': ['titl'],
};

/** Options for a single `layout()` call: things tied to that particular text. */
export interface LayoutExtras {
	/** Extra space between specific pairs of characters, e.g. `[{ at: 3, px: -4 }]`. */
	kerning?: Kerning[];
}

export function loadFont(url: string, faceIndex?: number): Promise<Font> {
	return Font.load(url, faceIndex);
}

/**
 * Text settings, measurement and layout. Layout itself runs in Rust; this
 * class holds the settings and maps them to layout options. It does not draw:
 * use `layout()` and render the glyphs however you like.
 * Units are px; y grows downward.
 */
export class Typography {
	private font: Font | null = null;
	private size = 12;
	private leading: number | null = null;
	private alignH: HorizAlign = START;
	private alignV: VertAlign = BASELINE;
	private wrap: WrapMode = WORD;
	private weight = 400;
	private weightSet = false;
	private style: TextStyle = NORMAL;
	private props: TextProperties = {
		fontKerning: 'auto',
		fontVariantCaps: 'normal',
		letterSpacing: 0,
		wordSpacing: 0,
		direction: 'inherit',
		fontFeatures: [],
		fontVariationSettings: {},
	};

	// -------------------------------------------------------------------------
	// State
	// -------------------------------------------------------------------------

	textFont(): Font | null;
	textFont(font: Font, size?: number): this;
	textFont(font?: Font, size?: number): Font | null | this {
		if (font === undefined) return this.font;
		this.font = font;
		if (size !== undefined) this.size = size;
		return this;
	}

	textSize(): number;
	textSize(size: number): this;
	textSize(size?: number): number | this {
		if (size === undefined) return this.size;
		this.size = size;
		return this;
	}

	/** Defaults to 1.25 × textSize until set. */
	textLeading(): number;
	textLeading(leading: number): this;
	textLeading(leading?: number): number | this {
		if (leading === undefined) return this.leading ?? this.size * 1.25;
		this.leading = leading;
		return this;
	}

	textAlign(): { horizontal: HorizAlign; vertical: VertAlign };
	textAlign(horizontal: HorizAlign, vertical?: VertAlign): this;
	textAlign(horizontal?: HorizAlign, vertical?: VertAlign) {
		if (horizontal === undefined) return { horizontal: this.alignH, vertical: this.alignV };
		this.alignH = horizontal;
		if (vertical !== undefined) this.alignV = vertical;
		return this;
	}

	/** Only takes effect when `layout()` gets a maxWidth. */
	textWrap(): WrapMode;
	textWrap(mode: WrapMode): this;
	textWrap(mode?: WrapMode): WrapMode | this {
		if (mode === undefined) return this.wrap;
		this.wrap = mode;
		return this;
	}

	textDirection(): TextDirectionSetting;
	textDirection(direction: TextDirectionSetting): this;
	textDirection(direction?: TextDirectionSetting): TextDirectionSetting | this {
		if (direction === undefined) return this.props.direction;
		this.props.direction = direction;
		return this;
	}

	/**
	 * Uses the `wght` axis of variable fonts; no effect on static fonts. With
	 * `textStyle(BOLD)` the weight is at least 700.
	 */
	textWeight(): number;
	textWeight(weight: number): this;
	textWeight(weight?: number): number | this {
		if (weight === undefined) return this.weight;
		this.weight = weight;
		this.weightSet = true;
		return this;
	}

	/**
	 * Uses real font data when possible (`wght`, `ital`, `slnt` axes); otherwise
	 * `layout().synthetic` says how to fake it.
	 */
	textStyle(): TextStyle;
	textStyle(style: TextStyle): this;
	textStyle(style?: TextStyle): TextStyle | this {
		if (style === undefined) return this.style;
		this.style = style;
		return this;
	}

	textProperty<K extends keyof TextProperties>(name: K): TextProperties[K];
	textProperty<K extends keyof TextProperties>(name: K, value: TextProperties[K]): this;
	textProperty<K extends keyof TextProperties>(name: K, value?: TextProperties[K]) {
		if (value === undefined) return this.props[name];
		this.props[name] = value;
		return this;
	}

	textProperties(): TextProperties;
	textProperties(values: Partial<TextProperties>): this;
	textProperties(values?: Partial<TextProperties>): TextProperties | this {
		if (values === undefined) return { ...this.props };
		Object.assign(this.props, values);
		return this;
	}

	// -------------------------------------------------------------------------
	// Metrics
	// -------------------------------------------------------------------------

	/** Loose ascent from the font's metrics, px. */
	fontAscent(): number {
		const m = this.requireFont().metrics(this.resolve().variations);
		return (m.ascender * this.size) / m.units_per_em;
	}

	/** Loose descent from the font's metrics, px (positive). */
	fontDescent(): number {
		const m = this.requireFont().metrics(this.resolve().variations);
		return (-m.descender * this.size) / m.units_per_em;
	}

	// The methods below that take a string accept the same `extras` as
	// `layout()`, e.g. `{ kerning }`, so they measure the text as it's laid out.

	/** Tight ascent of the first line of `str` if given, otherwise the font ascent. */
	textAscent(str?: string, extras?: LayoutExtras): number {
		if (str === undefined) return this.fontAscent();
		const line = this.layout(str, 0, 0, undefined, undefined, extras).lines[0];
		return line.ink ? Math.max(0, line.baseline - line.ink.y) : 0;
	}

	/** Tight descent of the first line of `str` if given, otherwise the font descent. */
	textDescent(str?: string, extras?: LayoutExtras): number {
		if (str === undefined) return this.fontDescent();
		const line = this.layout(str, 0, 0, undefined, undefined, extras).lines[0];
		return line.ink ? Math.max(0, line.ink.y + line.ink.h - line.baseline) : 0;
	}

	/** Loose (advance) width of the widest line, px. */
	fontWidth(str: string, extras?: LayoutExtras): number {
		const { lines } = this.layout(str, 0, 0, undefined, undefined, extras);
		return Math.max(0, ...lines.map(l => l.width));
	}

	/** Tight (ink) width of the widest line, px. */
	textWidth(str: string, extras?: LayoutExtras): number {
		const { lines } = this.layout(str, 0, 0, undefined, undefined, extras);
		return Math.max(0, ...lines.map(l => l.ink?.w ?? 0));
	}

	/** Loose box: advance widths × font ascent/descent per line. */
	fontBounds(
		str: string,
		x: number,
		y: number,
		maxWidth?: number,
		maxHeight?: number,
		extras?: LayoutExtras,
	): Bounds {
		return this.layout(str, x, y, maxWidth, maxHeight, extras).advance;
	}

	/** Tight box around the ink of every glyph. */
	textBounds(
		str: string,
		x: number,
		y: number,
		maxWidth?: number,
		maxHeight?: number,
		extras?: LayoutExtras,
	): Bounds {
		const layout = this.layout(str, x, y, maxWidth, maxHeight, extras);
		return layout.ink ?? { x: layout.lines[0].x, y: layout.lines[0].baseline, w: 0, h: 0 };
	}

	// -------------------------------------------------------------------------
	// Layout
	// -------------------------------------------------------------------------

	/**
	 * Positioned glyphs for `str`. Draw each glyph with
	 * `layout.font.path(g.id, layout.variations)` at (g.x, g.y), scaled by
	 * `layout.scale`, applying `layout.synthetic` if non-zero.
	 */
	layout(
		str: string,
		x: number,
		y: number,
		maxWidth?: number,
		maxHeight?: number,
		extras: LayoutExtras = {},
	): TextLayout {
		const font = this.requireFont();
		const { variations, synthetic } = this.resolve();
		const features = [
			...CAPS_FEATURES[this.props.fontVariantCaps],
			...this.props.fontFeatures,
		];
		if (this.props.fontKerning === 'none') features.push('kern=0');
		const result = font.layout(
			String(str),
			{
				x,
				y,
				size: this.size,
				leading: this.textLeading(),
				letter_spacing: toPx(this.props.letterSpacing),
				word_spacing: toPx(this.props.wordSpacing),
				kerning: extras.kerning ?? [],
				features,
				direction: this.props.direction === 'inherit' ? 'auto' : this.props.direction,
				wrap: this.wrap,
				align_h: this.alignH,
				align_v: this.alignV,
				max_width: maxWidth ?? null,
				max_height: maxHeight ?? null,
			},
			variations,
		);
		return { ...result, font, variations, synthetic };
	}

	// -------------------------------------------------------------------------
	// Internals
	// -------------------------------------------------------------------------

	private requireFont(): Font {
		if (!this.font) throw new Error('no font set; call textFont(font) first');
		return this.font;
	}

	/** Weight/style → axis values or faux styling (decided in Rust), plus raw axis overrides. */
	private resolve(): { variations: Variations; synthetic: Synthetic } {
		const resolved = this.requireFont().resolveStyle({
			weight: this.weightSet ? this.weight : null,
			bold: this.style === BOLD || this.style === BOLDITALIC,
			italic: this.style === ITALIC || this.style === BOLDITALIC,
		});
		return {
			variations: { ...resolved.variations, ...this.props.fontVariationSettings },
			synthetic: resolved.synthetic,
		};
	}
}

function toPx(v: number | string): number {
	if (typeof v === 'number') return v;
	const n = parseFloat(v);
	if (Number.isNaN(n) || !/^\s*-?[\d.]+(px)?\s*$/.test(v)) {
		throw new Error(`unsupported length ${JSON.stringify(v)}; use a number or "Npx"`);
	}
	return n;
}
