import "./style.css";
import {
  Font,
  Typography,
  initText,
  type Kerning,
  type TextLayout,
  type TextStyle,
} from "text-shape";
// Serve the separate .wasm file (smaller than the embedded copy, compiled while
// it downloads).
import wasmUrl from "../../js/wasm/text_shape_bg.wasm?url";
import { highlight, highlightAll } from "./highlight";
// Fonts from the repository's fonts/ folder; Vite copies them (and their
// licenses, linked from the footer) into the build.
import robotoFlexUrl from "../../fonts/RobotoFlex.ttf?url";
import robotoUrl from "../../fonts/Roboto-Regular.ttf?url";
import notoArabicUrl from "../../fonts/NotoSansArabic.ttf?url";
import robotoFlexLicense from "../../fonts/RobotoFlex-OFL.txt?url";
import robotoLicense from "../../fonts/Roboto-OFL.txt?url";
import notoArabicLicense from "../../fonts/NotoSansArabic-OFL.txt?url";

highlightAll();
await initText(wasmUrl);

const WIDTH = 680;
const HEIGHT = 380;
const BOX = { x: 40, y: 40, height: 300 };

const FONT_URLS: Record<string, string> = {
  flex: robotoFlexUrl,
  roboto: robotoUrl,
  arabic: notoArabicUrl,
};

for (const [id, href] of [
  ["license-roboto-flex", robotoFlexLicense],
  ["license-roboto", robotoLicense],
  ["license-noto-arabic", notoArabicLicense],
]) {
  document.getElementById(id)!.setAttribute("href", href);
}

const SAMPLES: Record<string, { text: string; font: string }> = {
  latin: {
    text: "Typography is the craft of arranging type to make written language legible and readable.",
    font: "flex",
  },
  arabic: {
    text: "مرحبا بالعالم. هذا نص عربي يلتف داخل الصندوق من اليمين إلى اليسار.",
    font: "arabic",
  },
};

const fonts = new Map<string, Promise<Font>>();
function getFont(key: string): Promise<Font> {
  let font = fonts.get(key);
  if (!font) {
    font = Font.load(FONT_URLS[key]);
    fonts.set(key, font);
  }
  return font;
}

const form = document.querySelector<HTMLFormElement>("#demo-form")!;
const canvas = document.querySelector<HTMLCanvasElement>("#demo")!;
const stats = document.querySelector<HTMLElement>("#demo-stats")!;
const code = document.querySelector<HTMLElement>("#demo-code")!;
const ctx = setupCanvas(canvas, WIDTH, HEIGHT);

const control = <T extends HTMLElement>(name: string) => form.elements.namedItem(name) as T;
function applySample() {
  const sample = SAMPLES[control<HTMLSelectElement>("sample").value];
  control<HTMLTextAreaElement>("text").value = sample.text;
  control<HTMLSelectElement>("font").value = sample.font;
}
applySample();
restoreFromHash();

/** Settings from the URL hash, e.g. #sample=arabic&style=bolditalic, so views can be shared. */
function restoreFromHash() {
  const params = new URLSearchParams(location.hash.slice(1));
  if (params.has("sample")) {
    control<HTMLSelectElement>("sample").value = params.get("sample")!;
    applySample();
  }
  for (const [name, value] of params) {
    const el = control<HTMLInputElement>(name);
    if (!el || name === "sample") continue;
    if (el.type === "checkbox") el.checked = value === "1";
    else el.value = value;
  }
}

function saveToHash(o: Record<string, string | number | boolean>) {
  const params = new URLSearchParams();
  for (const [name, value] of Object.entries(o)) {
    if (name === "text" && value === SAMPLES[o.sample as string]?.text) continue;
    params.set(name, typeof value === "boolean" ? (value ? "1" : "0") : String(value));
  }
  history.replaceState(null, "", `#${params}`);
}

let renderId = 0;
async function render() {
  const id = ++renderId;
  const o = values(form);
  const font = await getFont(o.font as string);
  if (id !== renderId) return; // a newer render started while the font loaded

  const size = o.size as number;
  const t = new Typography()
    .textFont(font, size)
    .textAlign(o.alignH as "left", o.alignV as "top")
    .textWrap(o.wrap as "word")
    .textLeading(size * (o.leading as number))
    .textWeight(o.weight as number)
    .textStyle(o.style as TextStyle)
    .textDirection(o.direction as "inherit")
    .textProperty("letterSpacing", o.spacing as number);
  const text = o.text as string;
  const boxWidth = o.width as number;
  const kerning = parseKerning(o.kerning as string);
  const layout = t.layout(text, BOX.x, BOX.y, boxWidth, BOX.height, { kerning });

  ctx.clearRect(0, 0, WIDTH, HEIGHT);
  drawBox(boxWidth);
  if (o.showLines) drawLines(layout);
  ctx.fillStyle = css("--fg");
  drawText(layout);
  if (o.showInk && layout.ink) {
    ctx.strokeStyle = css("--ink");
    ctx.lineWidth = 1;
    ctx.strokeRect(layout.ink.x, layout.ink.y, layout.ink.w, layout.ink.h);
  }
  if (o.showGlyphs) drawOrigins(layout);

  stats.textContent = describe(layout, font, o.weight as number);
  saveToHash(o);
  code.innerHTML = highlight(snippet(o, size, boxWidth, kerning), "ts");
}

/** The README's Canvas 2D drawing, with faux bold/italic. */
function drawText(layout: TextLayout) {
  const { font, scale, variations, synthetic } = layout;
  for (const line of layout.lines) {
    for (const g of line.glyphs) {
      if (!g.bbox) continue;
      const path = font.path(g.id, variations);
      ctx.save();
      ctx.translate(g.x, g.y);
      ctx.scale(scale, scale);
      if (synthetic.oblique) ctx.transform(1, 0, -synthetic.oblique, 1, 0, 0);
      ctx.fill(path);
      if (synthetic.bold) {
        ctx.lineWidth = synthetic.bold;
        ctx.lineJoin = "round";
        ctx.strokeStyle = ctx.fillStyle;
        ctx.stroke(path);
      }
      ctx.restore();
    }
  }
}

function drawBox(width: number) {
  ctx.save();
  ctx.strokeStyle = css("--muted");
  ctx.setLineDash([4, 3]);
  ctx.strokeRect(BOX.x + 0.5, BOX.y + 0.5, width, BOX.height);
  ctx.restore();
}

function drawLines(layout: TextLayout) {
  const accent = css("--accent");
  for (const line of layout.lines) {
    ctx.fillStyle = css("--line-fill");
    ctx.fillRect(line.x, line.baseline - layout.ascent, line.width, layout.ascent + layout.descent);
    ctx.fillStyle = accent;
    ctx.fillRect(line.x, line.baseline, line.width, 1);
  }
}

function drawOrigins(layout: TextLayout) {
  ctx.fillStyle = css("--accent");
  for (const g of layout.lines.flatMap((l) => l.glyphs)) {
    ctx.beginPath();
    ctx.arc(g.x, g.y, 2, 0, Math.PI * 2);
    ctx.fill();
  }
}

function describe(layout: TextLayout, font: Font, weight: number): string {
  const widest = Math.max(...layout.lines.map((l) => l.width));
  const parts = [
    `${layout.lines.length} ${layout.lines.length === 1 ? "line" : "lines"}`,
    `widest line ${widest.toFixed(1)}px`,
    `ink ${layout.ink ? `${layout.ink.w.toFixed(1)} × ${layout.ink.h.toFixed(1)}px` : "none"}`,
  ];
  const axes = Object.entries(layout.variations).map(([tag, v]) => `${tag} ${v}`);
  if (axes.length) parts.push(`axes: ${axes.join(", ")}`);
  if (layout.synthetic.bold) parts.push("faux bold");
  if (layout.synthetic.oblique) parts.push("faux italic");
  if (!font.axis("wght") && weight !== 400) parts.push("weight needs a wght axis");
  return parts.join(" · ");
}

/** The code for the current settings (defaults left out). */
function snippet(
  o: Record<string, string | number | boolean>,
  size: number,
  boxWidth: number,
  kerning: Kerning[],
) {
  const calls = [`.textFont(font, ${size})`];
  calls.push(`.textAlign(${String(o.alignH).toUpperCase()}, ${String(o.alignV).toUpperCase()})`);
  calls.push(`.textLeading(${+(size * (o.leading as number)).toFixed(2)})`);
  if (o.wrap !== "word") calls.push(`.textWrap(${String(o.wrap).toUpperCase()})`);
  if (o.weight !== 400) calls.push(`.textWeight(${o.weight})`);
  if (o.style !== "normal") calls.push(`.textStyle(${String(o.style).toUpperCase()})`);
  if (o.direction !== "inherit") calls.push(`.textDirection("${o.direction}")`);
  if (o.spacing !== 0) calls.push(`.textProperty("letterSpacing", ${o.spacing})`);
  return [
    "const t = new Typography()",
    ...calls.map((c) => `  ${c}`),
    "",
    `const layout = t.layout(text, ${BOX.x}, ${BOX.y}, ${boxWidth}, ${BOX.height}${
      kerning.length
        ? `, {\n  kerning: [${kerning.map((k) => `{ at: ${k.at}, px: ${k.px} }`).join(", ")}],\n}`
        : ""
    });`,
  ].join("\n");
}

/** "0:-4, 3:2" → [{ at: 0, px: -4 }, { at: 3, px: 2 }]; entries that don't parse are skipped. */
function parseKerning(input: string): Kerning[] {
  return input
    .split(",")
    .map((entry) => /^\s*(\d+)\s*:\s*(-?\d+(?:\.\d+)?)\s*$/.exec(entry))
    .filter((m): m is RegExpExecArray => m !== null)
    .map((m) => ({ at: Number(m[1]), px: Number(m[2]) }));
}

/** Size a canvas for the device pixel ratio; returns a context in CSS pixels. */
function setupCanvas(el: HTMLCanvasElement, width: number, height: number) {
  const dpr = window.devicePixelRatio || 1;
  el.width = Math.round(width * dpr);
  el.height = Math.round(height * dpr);
  // Height follows the width (CSS height: auto), so a narrow screen scales the
  // canvas down instead of squashing it.
  el.style.width = `${width}px`;
  const c = el.getContext("2d")!;
  c.setTransform(dpr, 0, 0, dpr, 0, 0);
  return c;
}

function css(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

/** Every named control in a form as a plain object (numbers as numbers). */
function values(f: HTMLFormElement): Record<string, string | number | boolean> {
  const out: Record<string, string | number | boolean> = {};
  for (const el of Array.from(f.elements) as HTMLInputElement[]) {
    if (!el.name) continue;
    if (el.type === "checkbox") out[el.name] = el.checked;
    else if (el.type === "range") out[el.name] = Number(el.value);
    else out[el.name] = el.value;
  }
  return out;
}

form.addEventListener("input", (e) => {
  if ((e.target as HTMLElement).getAttribute("name") === "sample") applySample();
  for (const out of form.querySelectorAll<HTMLOutputElement>("output[for]")) {
    const input = form.elements.namedItem(out.htmlFor.value) as HTMLInputElement | null;
    if (input) out.value = input.value;
  }
  void render();
});
form.addEventListener("submit", (e) => e.preventDefault());
form.dispatchEvent(new Event("input"));
