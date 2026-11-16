# text-shape

[![npm](https://img.shields.io/npm/v/text-shape)](https://www.npmjs.com/package/text-shape)
[![CI](https://github.com/yyyk/text-shape/actions/workflows/ci.yml/badge.svg)](https://github.com/yyyk/text-shape/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

Text layout and measurement: shaping, metrics, line breaking, bidi text, alignment and
bounds. It positions glyphs and hands you their outlines; rendering is up to you.

The engine is written in Rust on [rustybuzz](https://github.com/harfbuzz/rustybuzz) and
[ttf-parser](https://github.com/harfbuzz/ttf-parser), and is available for both Rust and
JavaScript. Try it in the [playground](https://yyyk.github.io/text-shape/).

| Package                                                         | For                                | Docs                                             |
| --------------------------------------------------------------- | ---------------------------------- | ------------------------------------------------ |
| [`text-shape`](https://crates.io/crates/text-shape) (crates.io) | Rust                               | [crates/text-shape](crates/text-shape/README.md) |
| [`text-shape`](https://www.npmjs.com/package/text-shape) (npm)  | Browsers and Node, via WebAssembly | [js](js/README.md)                               |

## Features

- Shaping with OpenType features, kerning, ligatures and variable font axes
- Font metrics, glyph bounds and outlines
- Line wrapping by word, where the Unicode line breaking rules allow it
  ([UAX #14](https://www.unicode.org/reports/tr14/)), or by character, without splitting emoji or accented letters
  ([UAX #29](https://www.unicode.org/reports/tr29/))
- Mixed left-to-right and right-to-left text, ordered by the Unicode bidirectional
  algorithm ([UAX #9](https://www.unicode.org/reports/tr9/))
- Horizontal and vertical alignment (start and end follow the text direction, as in
  CSS), leading, letter and word spacing
- Manual kerning of specific character pairs, and OpenType features on part of the
  text (e.g. kerning off for one word)
- Loose (advance) and tight (ink) widths and bounds
- Bold and italic through font axes, with faux bold/italic parameters as a fallback

## Quick look

Rust:

```rust
use text_shape::{Font, LayoutOptions};

let font = Font::from_bytes(std::fs::read("Roboto-Regular.ttf")?, 0)?;
let layout = font.layout("Hello world", &LayoutOptions {
    size: 48.0,
    max_width: Some(300.0),
    ..Default::default()
})?;
```

JavaScript:

```ts
import { loadFont, Typography } from 'text-shape';

const font = await loadFont('/fonts/Roboto-Regular.ttf');
const layout = new Typography().textFont(font, 48).layout('Hello world', 0, 0, 300);
```

Both return lines of positioned glyphs. See the package READMEs for drawing with
Canvas 2D and working with outlines.

## Repository layout

```
crates/text-shape/       layout engine (Rust crate)
crates/text-shape-wasm/  WebAssembly bindings used by the npm package
js/                      npm package: TypeScript API on top of the bindings
site/                    docs site and playground (GitHub Pages)
fonts/                   sample fonts (with licenses) for the tests and the site
```

## Development

Requires [rustup](https://rustup.rs) (Rust 1.85 or later) with the
`wasm32-unknown-unknown` target, [`wasm-pack`](https://github.com/wasm-bindgen/wasm-pack)
and Node.js 20 or later.

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
```

### Building

The Rust crate builds with Cargo from the repository root:

```sh
cargo build --release       # all crates; output in target/
cargo test                  # Rust tests and doctests
```

The npm package is built in `js/`, in two steps that `npm run build` runs together:

```sh
cd js && npm install
npm run build:wasm          # wasm-pack build of crates/text-shape-wasm into js/wasm/
npm run build:ts            # TypeScript into js/dist/
npm run dev                 # optional: rebuild js/dist/ on TypeScript changes
npm test                    # build, then run the JS tests
npm pack                    # optional: a .tgz to install elsewhere
```

The docs site and playground need the WASM module from `js/` (`npm run build:wasm`)
built first:

```sh
cd site && npm install
npm run dev                 # dev server
npm run build               # static site into site/dist/
npm run preview             # serve the built site
```

After changing Rust code, run `npm run build:wasm` in `js/` again; the site picks it
up on reload. The site uses the npm package's TypeScript source directly, so TypeScript
changes show up straight away.

Formatting: Rust uses rustfmt (`cargo fmt`); TypeScript, HTML, CSS, JSON and
Markdown use Prettier (`npm run format` in `js/` for the package and the Markdown, and
in `site/`). With the Prettier and rust-analyzer extensions, VS Code formats on save
(`.vscode/settings.json`).

CI (`.github/workflows/ci.yml`) runs all of this on every push, plus Rust 1.85 (the
minimum supported version), formatting, clippy and Node 20/22. To run the same checks
locally before pushing:

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings   # warnings fail CI
cargo test --workspace
(cd js && npm test && npm run format:check)
(cd site && npm run format:check && npm run build)
```

Changes are listed in each package's changelog:
[crate](crates/text-shape/CHANGELOG.md) and [npm](js/CHANGELOG.md).

### Third-party notices

The npm package ships the WebAssembly module, which has the Rust dependencies
compiled into it, so it must include their licenses. They're listed in
`THIRD_PARTY_NOTICES` at the repository root, which the package build copies into
`js/`. Nothing checks this file automatically.

Whenever you add, remove or upgrade a Rust dependency (including through
`cargo update`):

1. List the crates compiled into the module, with their licenses:

   ```sh
   cargo tree -p text-shape-wasm --target wasm32-unknown-unknown \
     -e normal,no-proc-macro --prefix none --no-dedupe --format "{p} {l}" | sort -u
   ```

   (Leave out `text-shape` and `text-shape-wasm` themselves.)

2. Compare with the entries in `THIRD_PARTY_NOTICES`, then add, remove or update
   them. For each crate, copy its license file from
   `~/.cargo/registry/src/*/<crate>-<version>/`: `LICENSE-MIT` (or the MIT
   `LICENSE`) when the crate offers MIT, otherwise the license it uses, plus any
   `NOTICE` file. `unicode-ident` also needs its `LICENSE-UNICODE`.

The Rust crate on crates.io doesn't need this file: Cargo downloads its
dependencies, licenses included, separately.

## Releasing

The crate and the npm package are versioned and released separately; release only
the one that changed. The npm package builds the WebAssembly from the Rust source in
this repository, so a Rust change that only matters to JavaScript users needs only an
npm release.

| Package | Version in                           | Changelog                        | Workflow                | Tag            |
| ------- | ------------------------------------ | -------------------------------- | ----------------------- | -------------- |
| Crate   | `Cargo.toml` (`[workspace.package]`) | `crates/text-shape/CHANGELOG.md` | **Release crate**       | `crate-vX.Y.Z` |
| npm     | `js/package.json`                    | `js/CHANGELOG.md`                | **Release npm package** | `npm-vX.Y.Z`   |

1. Set the version (for npm: `npm version x.y.z --no-git-tag-version` in `js/`).
2. In the changelog, turn `## [Unreleased]` into `## [x.y.z] - YYYY-MM-DD`.
3. For an npm release, if Rust dependencies changed since the last one, update
   `THIRD_PARTY_NOTICES` (see [Third-party notices](#third-party-notices)).
4. Merge to `main`, then run the workflow from the Actions tab.

Each workflow checks the changelog, runs the tests, creates the tag and GitHub
release, and publishes. See the comment at the top of
`.github/workflows/release-crate.yml` and `release-npm.yml` for the one-time
registry setup; each package's very first version is published by hand.

## License

MIT
