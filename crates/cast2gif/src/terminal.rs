use super::parser::CastEvent;

pub const ANSI_COLORS: [[u8; 3]; 16] = [
    [0x00, 0x00, 0x00], // 0 Black
    [0xaa, 0x00, 0x00], // 1 Red
    [0x00, 0xaa, 0x00], // 2 Green
    [0xaa, 0x55, 0x00], // 3 Yellow
    [0x00, 0x00, 0xaa], // 4 Blue
    [0xaa, 0x00, 0xaa], // 5 Magenta
    [0x00, 0xaa, 0xaa], // 6 Cyan
    [0xaa, 0xaa, 0xaa], // 7 White
    [0x55, 0x55, 0x55], // 8 Dark Gray
    [0xff, 0x55, 0x55], // 9 Light Red
    [0x55, 0xff, 0x55], // 10 Light Green
    [0xff, 0xff, 0x55], // 11 Light Yellow
    [0x55, 0x55, 0xff], // 12 Light Blue
    [0xff, 0x55, 0xff], // 13 Light Magenta
    [0x55, 0xff, 0xff], // 14 Light Cyan
    [0xff, 0xff, 0xff], // 15 Bright White
];

#[derive(Clone, Copy, PartialEq)]
pub struct CellAttr {
    pub fg: [u8; 3],
    pub bg: [u8; 3],
    pub bold: bool,
}

impl Default for CellAttr {
    fn default() -> Self {
        CellAttr { fg: [0xe1, 0xe4, 0xed], bg: [0x0a, 0x0e, 0x1a], bold: false }
    }
}

#[derive(Clone)]
pub struct Cell {
    pub ch: char,
    pub attr: CellAttr,
}

impl Default for Cell {
    fn default() -> Self {
        Cell { ch: ' ', attr: CellAttr::default() }
    }
}

pub struct Terminal {
    pub cols: usize,
    pub rows: usize,
    grid: Vec<Vec<Cell>>,
    pub cursor_x: usize,
    pub cursor_y: usize,
    attr: CellAttr,
    saved_cx: usize,
    saved_cy: usize,
}

impl Terminal {
    pub fn new(cols: usize, rows: usize) -> Self {
        let grid = vec![vec![Cell::default(); cols]; rows];
        Terminal {
            cols, rows, grid,
            cursor_x: 0, cursor_y: 0,
            attr: CellAttr::default(),
            saved_cx: 0, saved_cy: 0,
        }
    }

    pub fn cell(&self, x: usize, y: usize) -> &Cell {
        &self.grid[y][x]
    }

    fn put_char(&mut self, ch: char) {
        if self.cursor_y >= self.rows {
            return;
        }
        self.grid[self.cursor_y][self.cursor_x].ch = ch;
        self.grid[self.cursor_y][self.cursor_x].attr = self.attr;
        if self.cursor_x + 1 < self.cols {
            self.cursor_x += 1;
        }
    }

    fn scroll_up(&mut self) {
        self.grid.remove(0);
        self.grid.push(vec![Cell::default(); self.cols]);
    }

    fn newline(&mut self) {
        self.cursor_x = 0;
        if self.cursor_y + 1 >= self.rows {
            self.scroll_up();
        } else {
            self.cursor_y += 1;
        }
    }

    pub fn feed(&mut self, data: &str) {
        let mut chars = data.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '\x1b' {
                // ESC sequence
                if chars.peek() == Some(&'[') {
                    chars.next(); // consume '['
                    self.handle_csi(&mut chars);
                } else if chars.peek() == Some(&'7') {
                    chars.next();
                    self.saved_cx = self.cursor_x;
                    self.saved_cy = self.cursor_y;
                } else if chars.peek() == Some(&'8') {
                    chars.next();
                    self.cursor_x = self.saved_cx;
                    self.cursor_y = self.saved_cy;
                }
                continue;
            }

            match ch {
                '\n' => {
                    if self.cursor_y + 1 >= self.rows {
                        self.scroll_up();
                    } else {
                        self.cursor_y += 1;
                    }
                }
                '\r' => self.cursor_x = 0,
                '\t' => {
                    let spaces = 8 - (self.cursor_x % 8);
                    for _ in 0..spaces {
                        if self.cursor_x < self.cols {
                            self.put_char(' ');
                        }
                    }
                }
                '\x08' => {
                    if self.cursor_x > 0 { self.cursor_x -= 1; }
                }
                _ => self.put_char(ch),
            }
        }
    }

    fn handle_csi(&mut self, chars: &mut std::iter::Peekable<std::str::Chars>) {
        let mut params = Vec::new();
        let mut current = String::new();

        while let Some(&ch) = chars.peek() {
            if ch.is_ascii_digit() {
                current.push(ch);
                chars.next();
            } else if ch == ';' {
                params.push(current.parse::<u32>().unwrap_or(0));
                current.clear();
                chars.next();
            } else {
                params.push(current.parse::<u32>().unwrap_or(0));
                let cmd = chars.next().unwrap();
                self.exec_csi(&params, cmd);
                return;
            }
        }
        // Handle case where sequence ends without final char
        if !current.is_empty() {
            params.push(current.parse::<u32>().unwrap_or(0));
        }
    }

    fn exec_csi(&mut self, params: &[u32], cmd: char) {
        match cmd {
            'A' => { // Cursor up
                let n = std::cmp::max(params.get(0).copied().unwrap_or(1), 1) as usize;
                self.cursor_y = self.cursor_y.saturating_sub(n);
            }
            'B' => { // Cursor down
                let n = std::cmp::max(params.get(0).copied().unwrap_or(1), 1) as usize;
                self.cursor_y = (self.cursor_y + n).min(self.rows - 1);
            }
            'C' => { // Cursor forward
                let n = std::cmp::max(params.get(0).copied().unwrap_or(1), 1) as usize;
                self.cursor_x = (self.cursor_x + n).min(self.cols - 1);
            }
            'D' => { // Cursor back
                let n = std::cmp::max(params.get(0).copied().unwrap_or(1), 1) as usize;
                self.cursor_x = self.cursor_x.saturating_sub(n);
            }
            'H' | 'f' => { // Cursor position
                let row = std::cmp::max(params.get(0).copied().unwrap_or(1), 1) as usize;
                let col = std::cmp::max(params.get(1).copied().unwrap_or(1), 1) as usize;
                self.cursor_y = (row - 1).min(self.rows - 1);
                self.cursor_x = (col - 1).min(self.cols - 1);
            }
            'J' => { // Erase display
                let mode = params.get(0).copied().unwrap_or(0);
                match mode {
                    0 => {
                        // Clear from cursor to end
                        for x in self.cursor_x..self.cols {
                            self.grid[self.cursor_y][x] = Cell::default();
                        }
                        for y in (self.cursor_y + 1)..self.rows {
                            for x in 0..self.cols {
                                self.grid[y][x] = Cell::default();
                            }
                        }
                    }
                    1 => {
                        // Clear from start to cursor
                        for y in 0..self.cursor_y {
                            for x in 0..self.cols {
                                self.grid[y][x] = Cell::default();
                            }
                        }
                        for x in 0..=self.cursor_x {
                            self.grid[self.cursor_y][x] = Cell::default();
                        }
                    }
                    2 | 3 => {
                        // Clear all
                        for y in 0..self.rows {
                            for x in 0..self.cols {
                                self.grid[y][x] = Cell::default();
                            }
                        }
                    }
                    _ => {}
                }
            }
            'K' => { // Erase line
                let mode = params.get(0).copied().unwrap_or(0);
                match mode {
                    0 => {
                        for x in self.cursor_x..self.cols {
                            self.grid[self.cursor_y][x] = Cell::default();
                        }
                    }
                    1 => {
                        for x in 0..=self.cursor_x {
                            self.grid[self.cursor_y][x] = Cell::default();
                        }
                    }
                    2 => {
                        for x in 0..self.cols {
                            self.grid[self.cursor_y][x] = Cell::default();
                        }
                    }
                    _ => {}
                }
            }
            'm' => { // SGR (colors/styles)
                if params.is_empty() || (params.len() == 1 && params[0] == 0) {
                    self.attr = CellAttr::default();
                    return;
                }
                let mut i = 0;
                while i < params.len() {
                    match params[i] {
                        0 => self.attr = CellAttr::default(),
                        1 => self.attr.bold = true,
                        22 => self.attr.bold = false,
                        38 => {
                            // Extended foreground
                            if i + 1 < params.len() && params[i + 1] == 5 && i + 2 < params.len() {
                                let idx = params[i + 2] as usize;
                                if idx < 16 {
                                    self.attr.fg = ANSI_COLORS[idx];
                                } else if idx >= 232 {
                                    // Grayscale
                                    let v = ((idx - 232) * 10 + 8) as u8;
                                    self.attr.fg = [v, v, v];
                                } else {
                                    // 6x6x6 cube
                                    let idx = idx - 16;
                                    let r = ((idx / 36) * 40 + 55) as u8;
                                    let g = (((idx % 36) / 6) * 40 + 55) as u8;
                                    let b = ((idx % 6) * 40 + 55) as u8;
                                    self.attr.fg = [r, g, b];
                                }
                                i += 2;
                            }
                        }
                        48 => {
                            // Extended background
                            if i + 1 < params.len() && params[i + 1] == 5 && i + 2 < params.len() {
                                let idx = params[i + 2] as usize;
                                if idx < 16 {
                                    self.attr.bg = ANSI_COLORS[idx];
                                } else if idx >= 232 {
                                    let v = ((idx - 232) * 10 + 8) as u8;
                                    self.attr.bg = [v, v, v];
                                } else {
                                    let idx = idx - 16;
                                    let r = ((idx / 36) * 40 + 55) as u8;
                                    let g = (((idx % 36) / 6) * 40 + 55) as u8;
                                    let b = ((idx % 6) * 40 + 55) as u8;
                                    self.attr.bg = [r, g, b];
                                }
                                i += 2;
                            }
                        }
                        30..=37 => {
                            let idx = (params[i] - 30) as usize;
                            self.attr.fg = ANSI_COLORS[idx];
                        }
                        39 => self.attr.fg = CellAttr::default().fg,
                        40..=47 => {
                            let idx = (params[i] - 40) as usize;
                            self.attr.bg = ANSI_COLORS[idx];
                        }
                        49 => self.attr.bg = CellAttr::default().bg,
                        90..=97 => {
                            let idx = (params[i] - 90) as usize;
                            self.attr.fg = ANSI_COLORS[idx + 8];
                        }
                        100..=107 => {
                            let idx = (params[i] - 100) as usize;
                            self.attr.bg = ANSI_COLORS[idx + 8];
                        }
                        _ => {}
                    }
                    i += 1;
                }
            }
            's' => { // Save cursor
                self.saved_cx = self.cursor_x;
                self.saved_cy = self.cursor_y;
            }
            'u' => { // Restore cursor
                self.cursor_x = self.saved_cx;
                self.cursor_y = self.saved_cy;
            }
            'l' | 'h' => { // Hide/show cursor — ignore
            }
            _ => {} // Ignore unknown
        }
    }

    /// Feed all events up to (but not including) the given timestamp
    pub fn process_events(&mut self, events: &[CastEvent], up_to: f64) {
        for event in events {
            if event.timestamp >= up_to {
                break;
            }
            self.feed(&event.data);
        }
    }
}
