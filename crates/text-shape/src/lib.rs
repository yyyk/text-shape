//! Text shaping, metrics, Unicode segmentation and paragraph layout, built on
//! [HarfRust](https://docs.rs/harfrust) and [read-fonts](https://docs.rs/read-fonts).
//!
//! ```no_run
//! use text_shape::{Font, HorizAlign, LayoutOptions};
//!
//! let font = Font::from_bytes(std::fs::read("Roboto-Regular.ttf")?, 0)?;
//! let layout = font.layout(
//!     "Hello world, wrapped in a box",
//!     &LayoutOptions { size: 48.0, max_width: Some(400.0), align_h: HorizAlign::Center, ..Default::default() },
//! )?;
//! for line in &layout.lines {
//!     for g in &line.glyphs {
//!         let outline = font.outline(g.id); // font units, y up
//!         // draw at (g.x, g.y), scaling outline coordinates by layout.scale and flipping y
//!     }
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! Conventions:
//! - Text indices (glyph clusters, line ranges, break positions) are UTF-8 byte
//!   offsets into the input, so `&text[line.start..line.end]` works.
//! - Font values (metrics, advances, outlines, glyph bounds) are in font units with
//!   y up. Layout results are in px with y down; `scale` converts font units to px.
//!
//! Enable the `serde` feature for `Serialize`/`Deserialize` on the data types.

mod error;
mod font;
mod info;
mod layout;
mod outline;
mod segmentation;
mod shaping;

pub use error::{Error, Result};
pub use font::{
    Font, FontMetrics, FontStyle, GlyphBBox, ResolvedStyle, SYNTHETIC_BOLD_EM,
    SYNTHETIC_OBLIQUE_DEGREES, Synthetic, VariationAxis,
};
pub use info::{FontInfo, FontTable, NameEntry, NamedInstance};
pub use layout::{
    Bounds, HorizAlign, Kerning, LayoutLine, LayoutOptions, PositionedGlyph, TextLayout, VertAlign,
    WrapMode,
};
pub use outline::PathCmd;
pub use segmentation::{
    BaseDirection, BidiLine, BidiRun, LineBreak, bidi_line, graphemes, line_breaks,
};
pub use shaping::{Direction, ShapeOptions, ShapedGlyph, ShapingResult};

// Compiles the README example as a doctest.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
pub struct ReadmeDoctests;
