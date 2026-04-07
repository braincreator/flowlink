use super::parser::CastEvent;
use super::terminal::Terminal;
use super::vga_font::VGA_FONT;
use anyhow::Result;

pub struct Frame {
    pub rgba: Vec<u8>, // width * height * 4
    pub delay_ms: u16,
    pub width: u16,
    pub height: u16,
}

pub struct Renderer {
    pub cols: usize,
    pub rows: usize,
    pub width: usize,
    pub height: usize,
    pub bg: [u8; 3],
    pub fg: [u8; 3],
    char_w: usize,
    char_h: usize,
}

impl Renderer {
    pub fn new(cols: usize, rows: usize, bg: [u8; 3], fg: [u8; 3]) -> Self {
        let char_w = 8;
        let char_h = 16;
        Renderer {
            cols, rows,
            width: cols * char_w,
            height: rows * char_h,
            bg, fg,
            char_w, char_h,
        }
    }

    pub fn render_frames(
        &self,
        events: &[CastEvent],
        term: &mut Terminal,
        fps: u32,
        speed: f64,
    ) -> Result<Vec<Frame>> {
        let total_duration = events.last().map(|e| e.timestamp).unwrap_or(0.0);
        eprintln!("DEBUG: total_duration={total_duration:.2}s, fps={fps}, speed={speed}");
        if total_duration <= 0.0 {
            let rgba = self.render(term);
            return Ok(vec![Frame { rgba, delay_ms: 100, width: self.width as u16, height: self.height as u16 }]);
        }

        let frame_interval = 1.0 / (fps as f64) / speed;
        let num_frames = (total_duration / frame_interval).ceil() as usize + 1;
        eprintln!("DEBUG: frame_interval={frame_interval:.4}s, num_frames={num_frames}");

        let mut frames: Vec<Frame> = Vec::with_capacity(num_frames);
        let mut last_hash: u64 = 0;
        let mut pending_delay_ms: u16 = 0;

        for i in 0..num_frames {
            let target_time = (i as f64) * frame_interval;
            term.process_events(events, target_time);
            let rgba = self.render(term);

            let hash = self.hash_pixels(&rgba);
            if hash != last_hash {
                // Push accumulated delay to previous frame
                if pending_delay_ms > 0 {
                    if let Some(last) = frames.last_mut() {
                        last.delay_ms = last.delay_ms.saturating_add(pending_delay_ms);
                    }
                }
                pending_delay_ms = 0;

                let delay_ms = (frame_interval * 1000.0).round() as u16;
                frames.push(Frame { rgba, delay_ms, width: self.width as u16, height: self.height as u16 });
                last_hash = hash;
            } else {
                pending_delay_ms = pending_delay_ms.saturating_add((frame_interval * 1000.0).round() as u16);
            }
        }

        // Add final pause
        if let Some(last) = frames.last_mut() {
            last.delay_ms = last.delay_ms.saturating_add(200);
        }

        let total_ms: u32 = frames.iter().map(|f| f.delay_ms as u32).sum();
        eprintln!("DEBUG: {} frames, total delay={}ms ({:.2}s)", frames.len(), total_ms, total_ms as f64 / 1000.0);

        // Ensure at least one frame
        if frames.is_empty() {
            let rgba = self.render(term);
            frames.push(Frame { rgba, delay_ms: 100, width: self.width as u16, height: self.height as u16 });
        }

        Ok(frames)
    }

    fn render(&self, term: &Terminal) -> Vec<u8> {
        let mut buf = vec![0u8; self.width * self.height * 4];

        // Fill background
        for pixel in buf.chunks_exact_mut(4) {
            pixel[0] = self.bg[0];
            pixel[1] = self.bg[1];
            pixel[2] = self.bg[2];
            pixel[3] = 0xFF;
        }

        // Render each cell
        for y in 0..self.rows {
            for x in 0..self.cols {
                let cell = term.cell(x, y);
                if cell.ch == ' ' {
                    continue;
                }
                let (fg_r, fg_g, fg_b) = (cell.attr.fg[0], cell.attr.fg[1], cell.attr.fg[2]);
                let (bg_r, bg_g, bg_b) = (cell.attr.bg[0], cell.attr.bg[1], cell.attr.bg[2]);

                let char_idx = cell.ch as usize;
                if char_idx >= VGA_FONT.len() {
                    continue;
                }
                let glyph = &VGA_FONT[char_idx];

                let px_base_x = x * self.char_w;
                let py_base_y = y * self.char_h;

                for row in 0..self.char_h {
                    let bits = glyph[row];
                    for col in 0..self.char_w {
                        let px = px_base_x + col;
                        let py = py_base_y + row;
                        if px >= self.width || py >= self.height {
                            continue;
                        }
                        let offset = (py * self.width + px) * 4;
                        if (bits >> (7 - col)) & 1 != 0 {
                            buf[offset] = fg_r;
                            buf[offset + 1] = fg_g;
                            buf[offset + 2] = fg_b;
                        } else {
                            buf[offset] = bg_r;
                            buf[offset + 1] = bg_g;
                            buf[offset + 2] = bg_b;
                        }
                        buf[offset + 3] = 0xFF;
                    }
                }
            }
        }

        buf
    }

    fn hash_pixels(&self, buf: &[u8]) -> u64 {
        // FNV-1a hash over full buffer for reliable delta detection
        let mut h: u64 = 0xcbf29ce484222325;
        for &byte in buf.iter() {
            h ^= byte as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        h
    }
}
