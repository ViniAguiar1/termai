/// Placeholder for wgpu-based terminal renderer.
///
/// Responsibilities:
/// - Create GPU surface and pipeline
/// - Maintain glyph atlas (rasterized font glyphs)
/// - Render terminal grid as textured quads
/// - Handle resize events

pub struct Renderer {
    // TODO: wgpu device, queue, surface, pipeline, glyph atlas
}

impl Renderer {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}
