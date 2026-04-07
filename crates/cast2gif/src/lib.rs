pub mod encoder;
pub mod parser;
pub mod renderer;
pub mod terminal;
pub mod vga_font;

use anyhow::Result;
use std::path::Path;

pub struct ConvertOptions {
    pub cols: u32,
    pub rows: u32,
    pub fps: u32,
    pub speed: f64,
    pub bg: [u8; 3],
    pub fg: [u8; 3],
}

pub struct ConvertStats {
    pub frames: usize,
    pub duration_ms: f64,
    pub gif_size: usize,
}

pub fn convert(input: &Path, output: &Path, opts: ConvertOptions) -> Result<ConvertStats> {
    let events = parser::parse_cast(input)?;
    let mut term = terminal::Terminal::new(opts.cols as usize, opts.rows as usize);
    let renderer = renderer::Renderer::new(opts.cols as usize, opts.rows as usize, opts.bg, opts.fg);
    let frames = renderer.render_frames(&events.events, &mut term, opts.fps, opts.speed)?;
    let gif_size = encoder::encode_gif(&frames, output)?;
    let duration_ms = frames.last().map(|f| f.delay_ms as f64).unwrap_or(0.0);
    Ok(ConvertStats {
        frames: frames.len(),
        duration_ms,
        gif_size,
    })
}
