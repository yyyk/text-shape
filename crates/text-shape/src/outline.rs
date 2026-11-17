//! Glyph outlines.

use read_fonts::model::glyph::outline::{DrawMemory, DrawOptions, PathSink};
use read_fonts::types::GlyphId;

use crate::font::Font;

/// A path command in font units (y up).
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum PathCmd {
    MoveTo(f32, f32),
    LineTo(f32, f32),
    QuadTo(f32, f32, f32, f32),
    CubicTo(f32, f32, f32, f32, f32, f32),
    Close,
}

struct PathCollector<'a> {
    cmds: &'a mut Vec<PathCmd>,
}

impl<'a> PathSink for PathCollector<'a> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.cmds.push(PathCmd::MoveTo(x, y));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.cmds.push(PathCmd::LineTo(x, y));
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.cmds.push(PathCmd::QuadTo(x1, y1, x, y));
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.cmds.push(PathCmd::CubicTo(x1, y1, x2, y2, x, y));
    }
    fn close(&mut self) {
        self.cmds.push(PathCmd::Close);
    }
}

impl Font {
    /// Glyph outline in font units (y up). Empty for glyphs without an outline.
    /// Reflects current variations.
    pub fn outline(&self, glyph_id: u16) -> Vec<PathCmd> {
        let mut cmds = Vec::new();
        if let Some(glyph) = self
            .instance()
            .outline_glyphs()
            .get(GlyphId::new(glyph_id.into()))
        {
            // A glyph that fails to draw part way gets no outline, as before.
            if glyph
                .draw(
                    &DrawOptions::new(),
                    DrawMemory::default(),
                    &mut PathCollector { cmds: &mut cmds },
                )
                .is_err()
            {
                cmds.clear();
            }
        }
        cmds
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::test_font;

    #[test]
    fn outline_is_closed() {
        let cmds = test_font().outline(36);
        assert!(matches!(cmds.first(), Some(PathCmd::MoveTo(..))));
        assert!(matches!(cmds.last(), Some(PathCmd::Close)));
    }
}
