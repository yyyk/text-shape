// Tests the built package (dist/ + the embedded WASM) the way users load it.
// Layout itself is tested in Rust; these cover the JS layer: loading, Typography
// settings and how they map to layout options, the Font caches, UTF-16 indices.
import { readFile } from 'node:fs/promises';
import assert from 'node:assert/strict';
import { before, describe, test } from 'node:test';

import {
	BOLD,
	BOLDITALIC,
	CENTER,
	CHAR,
	END,
	Font,
	ITALIC,
	LEFT,
	RIGHT,
	TOP,
	Typography,
	initText,
} from '../dist/index.js';
import * as raw from '../wasm/text_shape.js';

const fontFile = name => readFile(new URL(`../../fonts/${name}`, import.meta.url));

let roboto, flex, arabic;
before(async () => {
	await initText();
	roboto = Font.fromBytes(await fontFile('Roboto-Regular.ttf'));
	flex = Font.fromBytes(await fontFile('RobotoFlex.ttf'));
	arabic = Font.fromBytes(await fontFile('NotoSansArabic.ttf'));
});

const typo = (font = roboto, size = 48) => new Typography().textFont(font, size);
const lineTexts = (text, layout) => layout.lines.map(l => text.slice(l.start, l.end));

describe('loading', () => {
	test('initText loads the embedded WASM once', () => {
		assert.equal(initText(), initText());
	});

	test('fonts expose metrics and axes', () => {
		assert.equal(roboto.metrics().family_name, 'Roboto');
		assert.deepEqual(roboto.axes, []);
		assert.deepEqual(
			flex.axis('wght') && [flex.axis('wght').min, flex.axis('wght').max],
			[100, 1000],
		);
	});

	test('errors surface as exceptions', () => {
		assert.throws(() => Font.fromBytes(new Uint8Array([1, 2, 3])), /could not parse font/);
		assert.throws(() => roboto.shape('a', { features: ['???'] }));
		assert.throws(() => raw.shape(99999, 'a', {}), /unknown font id/);
	});
});

describe('Typography settings', () => {
	test('getters return defaults', () => {
		const t = new Typography().textFont(roboto);
		assert.equal(t.textSize(), 12);
		assert.equal(t.textLeading(), 15, '1.25 × size until set');
		assert.deepEqual(t.textAlign(), { horizontal: 'start', vertical: 'baseline' });
		assert.equal(t.textWrap(), 'word');
		assert.equal(t.textStyle(), 'normal');
		assert.equal(t.textDirection(), 'inherit');
	});

	test('setters chain and apply to layout', () => {
		const t = typo().textAlign(CENTER, TOP).textLeading(70);
		const l = t.layout('a\nb', 0, 0, 400);
		assert.equal(l.lines[1].baseline - l.lines[0].baseline, 70);
		assert.ok(Math.abs(l.lines[0].baseline - t.fontAscent()) < 1e-3, 'TOP');
		assert.ok(
			Math.abs(l.lines[0].x + l.lines[0].width / 2 - 200) < 1e-3,
			'centered in the box',
		);
	});

	test("START and END follow each paragraph's direction", () => {
		const text = 'abc\nسلام'; // a left-to-right paragraph, then a right-to-left one
		const xs = t =>
			t
				.layout(text, 0, 0, 400)
				.lines.map(l => [l.direction, +l.x.toFixed(3), +(400 - l.width).toFixed(3)]);
		const [ltr, rtl] = xs(typo(arabic, 40)); // START is the default
		assert.deepEqual([ltr[0], ltr[1]], ['ltr', 0], 'LTR starts at the left edge');
		assert.deepEqual([rtl[0], rtl[1]], ['rtl', rtl[2]], 'RTL starts at the right edge');
		const [ltrEnd, rtlEnd] = xs(typo(arabic, 40).textAlign(END));
		assert.equal(ltrEnd[1], ltrEnd[2]);
		assert.equal(rtlEnd[1], 0);
		const [ltrLeft, rtlLeft] = xs(typo(arabic, 40).textAlign(LEFT));
		assert.equal(ltrLeft[1], 0);
		assert.equal(rtlLeft[1], 0, 'LEFT ignores direction');
	});

	test('requires a font', () => {
		assert.throws(() => new Typography().layout('a', 0, 0), /no font set/);
	});

	test('wraps by word, or by character', () => {
		const text = 'The quick brown fox jumps over the lazy dog';
		const words = typo().layout(text, 0, 0, 300);
		assert.deepEqual(lineTexts(text, words), [
			'The quick',
			'brown fox',
			'jumps over',
			'the lazy dog',
		]);
		assert.ok(words.lines.every(l => l.width <= 300));

		const chars = typo().textWrap(CHAR).layout('abcdefghij', 0, 0, 100);
		assert.ok(chars.lines.length > 1);
		assert.equal(lineTexts('abcdefghij', chars).join(''), 'abcdefghij');
	});

	test('fontKerning none turns kerning off', () => {
		const kerned = typo().fontWidth('AV');
		const t = typo().textProperty('fontKerning', 'none');
		assert.ok(t.fontWidth('AV') > kerned);
	});

	test('fontVariantCaps and fontFeatures become OpenType features', () => {
		const ids = (t, text) => t.layout(text, 0, 0).lines[0].glyphs.map(g => g.id);
		const plain = ids(typo(), 'abc');
		assert.notDeepEqual(
			ids(typo().textProperty('fontVariantCaps', 'small-caps'), 'abc'),
			plain,
		);
		assert.notDeepEqual(ids(typo().textProperty('fontFeatures', ['smcp']), 'abc'), plain);
		assert.equal(ids(typo(), 'fi').length, 1, 'fi ligature by default');
		assert.equal(ids(typo().textProperty('fontFeatures', ['liga=0']), 'fi').length, 2);
	});

	test('letterSpacing and wordSpacing accept numbers and px strings', () => {
		const plain = typo().fontWidth('ab c');
		assert.ok(
			Math.abs(
				typo().textProperty('letterSpacing', '2px').fontWidth('ab c') - plain - 8,
			) < 1e-3,
		);
		assert.ok(
			Math.abs(typo().textProperty('wordSpacing', 10).fontWidth('ab c') - plain - 10) <
				1e-3,
		);
		assert.throws(
			() => typo().textProperty('letterSpacing', '1em').layout('a', 0, 0),
			/unsupported length/,
		);
	});

	test('textDirection sets the paragraph base direction', () => {
		assert.equal(typo().layout('abc', 0, 0).lines[0].direction, 'ltr');
		assert.equal(typo().textDirection('rtl').layout('abc', 0, 0).lines[0].direction, 'rtl');
		assert.equal(
			typo(arabic).layout('سلام', 0, 0).lines[0].direction,
			'rtl',
			'inherit detects it',
		);
	});
});

describe('weight and style', () => {
	test('variable fonts use real axes', () => {
		const normal = typo(flex).layout('Hamburg', 0, 0);
		assert.deepEqual(normal.variations, { wght: 400 });

		const heavy = typo(flex).textWeight(900).layout('Hamburg', 0, 0);
		assert.deepEqual(heavy.variations, { wght: 900 });
		assert.ok(heavy.lines[0].width > normal.lines[0].width);

		// Bold means at least 700, even with a lighter weight set.
		assert.deepEqual(
			typo(flex).textWeight(400).textStyle(BOLD).layout('a', 0, 0).variations,
			{ wght: 700 },
		);
		assert.deepEqual(
			typo(flex).textWeight(900).textStyle(BOLD).layout('a', 0, 0).variations,
			{ wght: 900 },
		);

		const boldItalic = typo(flex).textStyle(BOLDITALIC).layout('Hamburg', 0, 0);
		assert.deepEqual(boldItalic.variations, { wght: 700, slnt: -10 });
		assert.deepEqual(boldItalic.synthetic, { bold: 0, oblique: 0 });
		assert.ok(boldItalic.ink, 'variable glyphs have outlines and bounds');
	});

	test('static fonts get faux bold and italic', () => {
		assert.deepEqual(typo().layout('a', 0, 0).synthetic, { bold: 0, oblique: 0 });
		const { synthetic, variations } = typo().textStyle(BOLD).layout('a', 0, 0);
		assert.ok(Math.abs(synthetic.bold - 2048 / 24) < 1e-3);
		assert.deepEqual(variations, {});
		assert.ok(
			Math.abs(typo().textStyle(ITALIC).layout('a', 0, 0).synthetic.oblique - 0.2493) <
				1e-3,
		);
	});

	test('fontVariationSettings override axes', () => {
		const narrow = typo(flex)
			.textProperty('fontVariationSettings', { wdth: 50 })
			.layout('Hamburg', 0, 0);
		assert.deepEqual(narrow.variations, { wght: 400, wdth: 50 });
		assert.ok(narrow.lines[0].width < typo(flex).fontWidth('Hamburg'));
	});

	test('switching weights between calls gives consistent results', () => {
		const light = typo(flex).textWeight(100);
		const heavy = typo(flex).textWeight(900);
		const first = [light.fontWidth('Hamburg'), heavy.fontWidth('Hamburg')];
		const again = [light.fontWidth('Hamburg'), heavy.fontWidth('Hamburg')];
		assert.deepEqual(again, first);
		assert.ok(first[0] < first[1]);
	});
});

describe('measurement', () => {
	test('loose and tight widths, bounds and ascent', () => {
		const t = typo();
		assert.ok(
			t.fontWidth('Hello') > t.textWidth('Hello'),
			'advance box includes side bearings',
		);
		const loose = t.fontBounds('Hag', 10, 20);
		const tight = t.textBounds('Hag', 10, 20);
		assert.ok(tight.x >= loose.x && tight.x + tight.w <= loose.x + loose.w + 1e-3);
		assert.ok(t.textAscent('Hag') <= t.fontAscent());
		assert.ok(t.textDescent('Hag') > 0, 'g descends');
		assert.ok(t.textDescent('Ha') < 1, 'only the overshoot of round letters');
	});

	test('empty and whitespace-only text', () => {
		const t = typo();
		assert.equal(t.layout('', 0, 0).lines.length, 1);
		assert.equal(t.textWidth('   '), 0);
		assert.deepEqual(t.textBounds('   ', 5, 6).w, 0);
	});

	test('measurements take the same extras as layout()', () => {
		const t = typo();
		const extras = { kerning: [{ at: 0, px: 10 }] }; // between H and e
		const layout = t.layout('Hello', 3, 4, 300, 200, extras);
		assert.ok(Math.abs(t.fontWidth('Hello', extras) - t.fontWidth('Hello') - 10) < 1e-3);
		assert.ok(Math.abs(t.textWidth('Hello', extras) - t.textWidth('Hello') - 10) < 1e-3);
		assert.deepEqual(t.fontBounds('Hello', 3, 4, 300, 200, extras), layout.advance);
		assert.deepEqual(t.textBounds('Hello', 3, 4, 300, 200, extras), layout.ink);
		assert.equal(
			t.textAscent('Hello', extras),
			t.textAscent('Hello'),
			'kerning is horizontal',
		);
		assert.equal(t.textDescent('Hello', extras), t.textDescent('Hello'));
	});

	test('alignment moves the bounds', () => {
		const right = typo().textAlign(RIGHT).fontBounds('Hello', 100, 0);
		assert.ok(Math.abs(right.x + right.w - 100) < 1e-3);
	});
});

describe('indices are UTF-16 offsets', () => {
	test('line ranges and clusters slice the JS string', () => {
		const text = 'a😀 é\nnext';
		const l = typo().layout(text, 0, 0);
		assert.deepEqual(lineTexts(text, l), ['a😀 é', 'next']);
		const clusters = l.lines[0].glyphs.map(g => g.cluster);
		assert.deepEqual(clusters, [0, 1, 3, 4], 'the emoji takes two UTF-16 units');
	});

	test('manual kerning positions are UTF-16 offsets', () => {
		const text = '😀AVA'; // A is at 2, V at 3
		const glyphs = extras =>
			typo().layout(text, 0, 0, undefined, undefined, extras).lines[0].glyphs;
		const plain = glyphs();
		const kerned = glyphs({ kerning: [{ at: 2, px: 10 }] }); // between A and V
		const dx = kerned.map((g, i) => g.x - plain[i].x);
		assert.ok(Math.abs(dx[1]) < 1e-3, 'A stays');
		assert.ok(Math.abs(dx[2] - 10) < 1e-3, 'V moves right by 10');
		assert.ok(Math.abs(dx[3] - 10) < 1e-3, 'and so does everything after it');
	});

	test('feature ranges are UTF-16 offsets, in any paragraph', () => {
		const width = (text, features) =>
			typo().textProperty('fontFeatures', features).layout(text, 0, 0).lines.at(-1).width;
		const text = '😀\nAV'; // the second paragraph's AV is at 3..5
		assert.ok(width(text, ['kern[3:5]=0']) > width(text, []));
		assert.equal(width(text, ['kern[3:5]=0']), width(text, ['kern=0']));
		assert.equal(
			width(text, ['kern[0:3]=0']),
			width(text, []),
			'a range before it changes nothing',
		);
		assert.ok(
			roboto.shape('😀AV', { features: ['kern[2:4]=0'] }).glyphs[1].x_advance >
				roboto.shape('😀AV').glyphs[1].x_advance,
			'shape() converts ranges too',
		);
	});

	test('low-level segmentation agrees', () => {
		assert.deepEqual(Array.from(raw.graphemes('a👨‍👩‍👧b')), [0, 1, 9, 10]);
		assert.deepEqual(
			raw.line_breaks('😀 a').map(b => b.index),
			[3, 4],
		);
	});
});

describe('Font caches', () => {
	test('outlines are cached per glyph and variations', () => {
		const id = flex.shape('H').glyphs[0].id;
		assert.equal(flex.outline(id), flex.outline(id));
		assert.notDeepEqual(flex.outline(id, { wght: 900 }), flex.outline(id, { wght: 100 }));
		assert.deepEqual(
			flex.outline(id, { wght: 5000 }),
			flex.outline(id, { wght: 1000 }),
			'clamped',
		);
	});

	test('path() builds and caches a Path2D', () => {
		const calls = [];
		globalThis.Path2D = class {
			moveTo(...a) {
				calls.push(['M', ...a]);
			}
			lineTo(...a) {
				calls.push(['L', ...a]);
			}
			quadraticCurveTo(...a) {
				calls.push(['Q', ...a]);
			}
			bezierCurveTo(...a) {
				calls.push(['C', ...a]);
			}
			closePath() {
				calls.push(['Z']);
			}
		};
		try {
			const id = roboto.shape('H').glyphs[0].id;
			const path = roboto.path(id);
			assert.equal(roboto.path(id), path);
			assert.equal(calls[0][0], 'M');
			assert.equal(calls.at(-1)[0], 'Z');
			const [, , y] = calls[0];
			const outlineY = roboto.outline(id)[0].MoveTo[1];
			assert.equal(y, -outlineY, 'y is flipped for canvas');
		} finally {
			delete globalThis.Path2D;
		}
	});
});
