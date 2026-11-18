import initWasm, * as wasm from '../wasm/text_shape.js';
import type {
	FontMetrics,
	FontStyle,
	InitInput,
	LayoutOptions,
	LayoutResult,
	PathCommand,
	ResolvedStyle,
	ShapeOptions,
	ShapingResult,
	VariationAxis,
} from '../wasm/text_shape.js';

export type Variations = Record<string, number>;

let wasmReady: Promise<unknown> | null = null;

/**
 * Initializes the wasm module once. Safe to call repeatedly; only the first
 * call's `wasm` argument is used.
 *
 * By default the binary embedded in the package is used, so no bundler or
 * server setup is needed. To serve the .wasm file yourself instead (smaller
 * download, streaming compile), pass its URL, a Response or the bytes. With
 * Vite: `import wasmUrl from "text-shape/text_shape_bg.wasm?url"; initText(wasmUrl);`
 */
export function initText(wasm?: InitInput | Promise<InitInput>): Promise<unknown> {
	return (wasmReady ??= initWasm({ module_or_path: wasm ?? embeddedWasm() }));
}

async function embeddedWasm(): Promise<Uint8Array> {
	const { default: base64 } = await import('../wasm/text_shape_wasm_base64.js');
	const binary = atob(base64);
	const bytes = new Uint8Array(binary.length);
	for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
	return bytes;
}

/**
 * A loaded font. Wraps the wasm font id and caches metrics, outlines and
 * Path2Ds per variation setting.
 *
 * Variations are stateful on the wasm side, so every call takes the variations
 * it wants and the font re-applies them only when they change.
 */
export class Font {
	readonly id: number;
	readonly axes: VariationAxis[];

	private appliedKey = '';
	private metricsCache = new Map<string, FontMetrics>();
	private styleCache = new Map<string, ResolvedStyle>();
	private outlineCache = new Map<string, PathCommand[]>();
	private pathCache = new Map<string, Path2D>();

	private constructor(id: number) {
		this.id = id;
		this.axes = wasm.get_variation_axes(id);
	}

	static async load(url: string, faceIndex?: number): Promise<Font> {
		const [buf] = await Promise.all([
			fetch(url).then(r => {
				if (!r.ok) throw new Error(`failed to load font ${url}: ${r.status}`);
				return r.arrayBuffer();
			}),
			initText(),
		]);
		return new Font(wasm.load_font(new Uint8Array(buf), faceIndex));
	}

	/** Requires `initText()` to have resolved. */
	static fromBytes(bytes: Uint8Array, faceIndex?: number): Font {
		return new Font(wasm.load_font(bytes, faceIndex));
	}

	axis(tag: string): VariationAxis | undefined {
		return this.axes.find(a => a.tag === tag);
	}

	metrics(variations: Variations = {}): FontMetrics {
		const key = this.apply(variations);
		let m = this.metricsCache.get(key);
		if (!m) {
			m = wasm.get_metrics(this.id);
			this.metricsCache.set(key, m);
		}
		return m;
	}

	shape(
		text: string,
		options: ShapeOptions = {},
		variations: Variations = {},
	): ShapingResult {
		this.apply(variations);
		return wasm.shape(this.id, text, options);
	}

	/** Lines and positioned glyphs. Low-level; `Typography.layout()` builds the options for you. */
	layout(
		text: string,
		options: LayoutOptions = {},
		variations: Variations = {},
	): LayoutResult {
		this.apply(variations);
		return wasm.layout(this.id, text, options);
	}

	/** Maps a weight/style onto this font's axes, or faux bold/italic parameters. */
	resolveStyle(style: FontStyle = {}): ResolvedStyle {
		const key = `${style.weight ?? ''}|${!!style.bold}|${!!style.italic}`;
		let resolved = this.styleCache.get(key);
		if (!resolved) {
			resolved = wasm.resolve_style(this.id, style);
			this.styleCache.set(key, resolved);
		}
		return resolved;
	}

	/** Glyph outline in font units, y up, or y down with `flipY`. */
	outline(
		glyphId: number,
		variations: Variations = {},
		{ flipY = false } = {},
	): PathCommand[] {
		const key = `${glyphId}|${this.apply(variations)}|${flipY}`;
		let cmds = this.outlineCache.get(key);
		if (!cmds) {
			cmds = wasm.get_outline(this.id, glyphId, flipY);
			this.outlineCache.set(key, cmds);
		}
		return cmds;
	}

	/** Glyph path in font units with y flipped to canvas orientation (y down). */
	path(glyphId: number, variations: Variations = {}): Path2D {
		const key = `${glyphId}|${variationKey(this.clamp(variations))}`;
		let path = this.pathCache.get(key);
		if (!path) {
			path = outlineToPath2D(this.outline(glyphId, variations, { flipY: true }));
			this.pathCache.set(key, path);
		}
		return path;
	}

	dispose(): void {
		wasm.unload_font(this.id);
		this.metricsCache.clear();
		this.styleCache.clear();
		this.outlineCache.clear();
		this.pathCache.clear();
	}

	/** Drops unknown axes and clamps values to each axis range. */
	private clamp(variations: Variations): Variations {
		const out: Variations = {};
		for (const [tag, value] of Object.entries(variations)) {
			const axis = this.axis(tag);
			if (axis) out[tag] = Math.min(axis.max, Math.max(axis.min, value));
		}
		return out;
	}

	private apply(variations: Variations): string {
		const clamped = this.clamp(variations);
		const key = variationKey(clamped);
		if (key !== this.appliedKey) {
			wasm.reset_variations(this.id);
			if (key) wasm.set_variations(this.id, clamped);
			this.appliedKey = key;
		}
		return key;
	}
}

function variationKey(v: Variations): string {
	return Object.keys(v)
		.sort()
		.map(tag => `${tag}=${v[tag]}`)
		.join(',');
}

function outlineToPath2D(cmds: PathCommand[]): Path2D {
	const p = new Path2D();
	for (const cmd of cmds) {
		switch (cmd.type) {
			case 'moveTo':
				p.moveTo(cmd.x, cmd.y);
				break;
			case 'lineTo':
				p.lineTo(cmd.x, cmd.y);
				break;
			case 'quadTo':
				p.quadraticCurveTo(cmd.cx, cmd.cy, cmd.x, cmd.y);
				break;
			case 'cubicTo':
				p.bezierCurveTo(cmd.c1x, cmd.c1y, cmd.c2x, cmd.c2y, cmd.x, cmd.y);
				break;
			case 'close':
				p.closePath();
				break;
		}
	}
	return p;
}
