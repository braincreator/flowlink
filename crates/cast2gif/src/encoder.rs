use super::renderer::Frame;
use anyhow::Result;
use std::collections::HashMap;
use std::io::BufWriter;
use std::path::Path;

/// Simple median-cut-ish quantizer: collect unique colors, map to palette
struct ColorQuantizer {
    palette: Vec<[u8; 3]>,
    color_to_idx: HashMap<[u8; 3], u8>,
}

impl ColorQuantizer {
    fn new() -> Self {
        ColorQuantizer {
            palette: Vec::with_capacity(256),
            color_to_idx: HashMap::with_capacity(256),
        }
    }

    fn add_color(&mut self, r: u8, g: u8, b: u8) -> u8 {
        if let Some(&idx) = self.color_to_idx.get(&[r, g, b]) {
            return idx;
        }
        let idx = self.palette.len() as u8;
        if self.palette.len() >= 256 {
            // Find nearest color
            return self.find_nearest(r, g, b);
        }
        self.palette.push([r, g, b]);
        self.color_to_idx.insert([r, g, b], idx);
        idx
    }

    fn find_nearest(&self, r: u8, g: u8, b: u8) -> u8 {
        let mut best_idx = 0u8;
        let mut best_dist = i32::MAX;
        for (i, &c) in self.palette.iter().enumerate() {
            let dr = (r as i32 - c[0] as i32).abs();
            let dg = (g as i32 - c[1] as i32).abs();
            let db = (b as i32 - c[2] as i32).abs();
            let dist = dr * dr + dg * dg + db * db;
            if dist < best_dist {
                best_dist = dist;
                best_idx = i as u8;
            }
        }
        best_idx
    }
}

pub fn encode_gif(frames: &[Frame], path: &Path) -> Result<usize> {
    let (width, height) = (frames[0].width, frames[0].height);

    let file = std::fs::File::create(path)?;
    let w = BufWriter::new(file);
    let mut encoder = gif::Encoder::new(w, width, height, &[])?;
    encoder.set_repeat(gif::Repeat::Infinite)?;

    let mut frame_buf: Vec<u8> = vec![0; width as usize * height as usize];

    for frame in frames {
        let mut quant = ColorQuantizer::new();

        // First pass: build palette
        for chunk in frame.rgba.chunks_exact(4) {
            quant.add_color(chunk[0], chunk[1], chunk[2]);
        }

        // Second pass: quantize
        for (i, chunk) in frame.rgba.chunks_exact(4).enumerate() {
            frame_buf[i] = quant.add_color(chunk[0], chunk[1], chunk[2]);
        }

        // Build color table
        let mut palette = vec![0u8; 256 * 3];
        for (i, color) in quant.palette.iter().enumerate() {
            palette[i * 3] = color[0];
            palette[i * 3 + 1] = color[1];
            palette[i * 3 + 2] = color[2];
        }

        let mut gif_frame = gif::Frame {
            width,
            height,
            buffer: std::borrow::Cow::Owned(frame_buf.clone()),
            palette: Some(palette),
            delay: frame.delay_ms / 10,
            ..Default::default()
        };
        if gif_frame.delay == 0 {
            gif_frame.delay = 1;
        }
        encoder.write_frame(&gif_frame)?;
    }

    drop(encoder);
    let gif_size = std::fs::metadata(path)?.len() as usize;
    Ok(gif_size)
}
