//! A small flow-layout engine: headings, wrapped paragraphs, tables that
//! break across pages (repeating their header) and embedded SVG. It produces
//! a display list per page, so page numbers are known before the PDF is
//! written (for the contents page and "page n of N").

use std::sync::OnceLock;

pub type Rgb = [u8; 3];

pub const TEXT: Rgb = [17, 24, 39];
pub const SUBTEXT: Rgb = [100, 110, 125];
pub const RULE: Rgb = [208, 213, 221];
pub const ZEBRA: Rgb = [246, 247, 249];
pub const HEAD_BG: Rgb = [234, 238, 245];
pub const ERROR: Rgb = [220, 38, 38];
pub const WARN: Rgb = [217, 119, 6];
pub const OK: Rgb = [22, 163, 74];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontKind {
    Regular,
    Bold,
    Mono,
}

pub(crate) const REGULAR: &[u8] = include_bytes!("../fonts/DejaVuSansCondensed.ttf");
pub(crate) const BOLD: &[u8] = include_bytes!("../fonts/DejaVuSansCondensed-Bold.ttf");
pub(crate) const MONO: &[u8] = include_bytes!("../fonts/DejaVuSansMono.ttf");

impl FontKind {
    pub(crate) fn data(self) -> &'static [u8] {
        match self {
            FontKind::Regular => REGULAR,
            FontKind::Bold => BOLD,
            FontKind::Mono => MONO,
        }
    }
}

struct Metrics {
    upem: f32,
    /// Advance per char for the BMP, in font units; 0 = no glyph.
    advances: Vec<u16>,
    fallback: u16,
}

fn metrics(kind: FontKind) -> &'static Metrics {
    static M: [OnceLock<Metrics>; 3] = [OnceLock::new(), OnceLock::new(), OnceLock::new()];
    M[kind as usize].get_or_init(|| {
        let face = ttf_parser::Face::parse(kind.data(), 0).expect("bundled font parses");
        let adv = |c: char| face.glyph_index(c).and_then(|g| face.glyph_hor_advance(g)).unwrap_or(0);
        let advances = (0u32..0x1_0000).map(|u| char::from_u32(u).map(adv).unwrap_or(0)).collect();
        Metrics { upem: face.units_per_em() as f32, advances, fallback: adv('?') }
    })
}

/// Width of `text` in points.
pub fn text_width(kind: FontKind, size: f32, text: &str) -> f32 {
    let m = metrics(kind);
    let units: u32 = text
        .chars()
        .map(|c| match m.advances.get(c as usize) {
            Some(&a) if a > 0 => a as u32,
            _ => m.fallback as u32,
        })
        .sum();
    units as f32 / m.upem * size
}

/// Whether the font can draw every character (used by tests).
pub fn covers(kind: FontKind, text: &str) -> bool {
    let m = metrics(kind);
    text.chars().all(|c| c.is_whitespace() || m.advances.get(c as usize).is_some_and(|a| *a > 0))
}

/// Greedy word wrap; words longer than the line are broken by character.
pub fn wrap(kind: FontKind, size: f32, text: &str, width: f32) -> Vec<String> {
    let mut lines = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        for word in para.split(' ') {
            let candidate = if line.is_empty() { word.to_owned() } else { format!("{line} {word}") };
            if text_width(kind, size, &candidate) <= width {
                line = candidate;
                continue;
            }
            if !line.is_empty() {
                lines.push(std::mem::take(&mut line));
            }
            // Break an over-long word.
            let mut chunk = String::new();
            for c in word.chars() {
                chunk.push(c);
                if text_width(kind, size, &chunk) > width && chunk.chars().count() > 1 {
                    let last = chunk.pop().expect("non-empty");
                    lines.push(std::mem::take(&mut chunk));
                    chunk.push(last);
                }
            }
            line = chunk;
        }
        lines.push(line);
    }
    lines
}

#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    /// `y` is the baseline.
    Text { x: f32, y: f32, size: f32, font: FontKind, color: Rgb, text: String },
    Rect { x: f32, y: f32, w: f32, h: f32, fill: Option<Rgb>, stroke: Option<(Rgb, f32)> },
    Line { x1: f32, y1: f32, x2: f32, y2: f32, color: Rgb, width: f32 },
    Svg { svg: String, x: f32, y: f32, w: f32, h: f32 },
    /// Clickable area that jumps to a page (contents entries).
    Link { x: f32, y: f32, w: f32, h: f32, page: usize },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    pub width: f32,
    pub height: f32,
    pub ops: Vec<Op>,
    /// Cover and contents pages carry no running header.
    pub plain: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TocEntry {
    pub title: String,
    pub level: u8,
    pub page: usize,
    pub y: f32,
}

pub const A4: (f32, f32) = (595.28, 841.89);
pub const MARGIN_X: f32 = 48.0;
pub const TOP: f32 = 64.0;
pub const BOTTOM: f32 = 56.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    #[default]
    Left,
    Right,
}

#[derive(Debug, Clone)]
pub struct Col {
    pub title: String,
    /// Share of the table width.
    pub weight: f32,
    pub mono: bool,
    pub align: Align,
}

pub fn col(title: &str, weight: f32) -> Col {
    Col { title: title.into(), weight, mono: false, align: Align::Left }
}

pub fn mono(title: &str, weight: f32) -> Col {
    Col { title: title.into(), weight, mono: true, align: Align::Left }
}

pub fn num(title: &str, weight: f32) -> Col {
    Col { title: title.into(), weight, mono: false, align: Align::Right }
}

#[derive(Debug, Clone, Default)]
pub struct Cell {
    pub text: String,
    pub color: Option<Rgb>,
    pub bold: bool,
}

impl From<String> for Cell {
    fn from(text: String) -> Self {
        Cell { text, ..Default::default() }
    }
}

impl From<&str> for Cell {
    fn from(text: &str) -> Self {
        Cell { text: text.into(), ..Default::default() }
    }
}

pub fn colored(text: impl Into<String>, color: Rgb) -> Cell {
    Cell { text: text.into(), color: Some(color), bold: false }
}

pub fn bold(text: impl Into<String>) -> Cell {
    Cell { text: text.into(), color: None, bold: true }
}

pub struct Doc {
    pub pages: Vec<Page>,
    pub toc: Vec<TocEntry>,
    y: f32,
}

const TABLE_SIZE: f32 = 8.0;
const LINE: f32 = 1.25;
const CELL_PAD: f32 = 3.5;

impl Default for Doc {
    fn default() -> Self {
        Self::new()
    }
}

impl Doc {
    pub fn new() -> Self {
        Self { pages: Vec::new(), toc: Vec::new(), y: TOP }
    }

    pub fn page_mut(&mut self) -> &mut Page {
        if self.pages.is_empty() {
            self.new_page(false);
        }
        self.pages.last_mut().expect("at least one page")
    }

    fn size(&self) -> (f32, f32) {
        self.pages.last().map(|p| (p.width, p.height)).unwrap_or(A4)
    }

    pub fn content_width(&self) -> f32 {
        self.size().0 - 2.0 * MARGIN_X
    }

    fn bottom(&self) -> f32 {
        self.size().1 - BOTTOM
    }

    pub fn remaining(&self) -> f32 {
        self.bottom() - self.y
    }

    pub fn new_page(&mut self, landscape: bool) {
        let (w, h) = if landscape { (A4.1, A4.0) } else { A4 };
        self.pages.push(Page { width: w, height: h, ops: Vec::new(), plain: false });
        self.y = TOP;
    }

    pub fn new_plain_page(&mut self) {
        self.new_page(false);
        self.page_mut().plain = true;
    }

    /// Start a new page if less than `h` points are left.
    pub fn ensure(&mut self, h: f32) {
        if self.pages.is_empty() || self.remaining() < h {
            let landscape = self.pages.last().is_some_and(|p| p.width > p.height);
            self.new_page(landscape);
        }
    }

    pub fn push(&mut self, op: Op) {
        self.page_mut().ops.push(op);
    }

    pub fn y(&self) -> f32 {
        self.y
    }

    pub fn set_y(&mut self, y: f32) {
        self.y = y;
    }

    pub fn space(&mut self, h: f32) {
        self.y += h;
    }

    pub fn text_at(&mut self, x: f32, y: f32, size: f32, font: FontKind, color: Rgb, text: impl Into<String>) {
        self.push(Op::Text { x, y, size, font, color, text: text.into() });
    }

    /// Numbered top-level section on a fresh page.
    pub fn section(&mut self, number: usize, title: &str, landscape: bool) {
        self.new_page(landscape);
        let page = self.pages.len() - 1;
        self.toc.push(TocEntry { title: format!("{number}  {title}"), level: 0, page, y: self.y });
        let y = self.y + 18.0;
        self.text_at(MARGIN_X, y, 18.0, FontKind::Bold, TEXT, format!("{number}  {title}"));
        let w = self.content_width();
        self.push(Op::Rect { x: MARGIN_X, y: y + 7.0, w: 36.0, h: 2.5, fill: Some(netvis_core::style::ACCENT), stroke: None });
        let _ = w;
        self.y = y + 22.0;
    }

    /// Second-level heading, kept with at least `keep` points of what follows.
    pub fn subsection(&mut self, title: &str, keep: f32) {
        self.space(6.0);
        self.ensure(28.0 + keep);
        let page = self.pages.len() - 1;
        self.toc.push(TocEntry { title: title.into(), level: 1, page, y: self.y });
        self.text_at(MARGIN_X, self.y + 12.0, 12.0, FontKind::Bold, TEXT, title);
        self.y += 20.0;
    }

    pub fn paragraph(&mut self, text: &str, size: f32, color: Rgb) {
        let w = self.content_width();
        for line in wrap(FontKind::Regular, size, text, w) {
            self.ensure(size * LINE);
            self.y += size * LINE;
            self.text_at(MARGIN_X, self.y - size * 0.25, size, FontKind::Regular, color, line);
        }
        self.y += size * 0.5;
    }

    /// Two-column key/value list.
    pub fn kv(&mut self, rows: &[(&str, String)]) {
        let size = 9.5;
        let key_w = rows.iter().map(|(k, _)| text_width(FontKind::Regular, size, k)).fold(0.0, f32::max) + 16.0;
        let val_w = self.content_width() - key_w;
        for (k, v) in rows {
            let lines = wrap(FontKind::Regular, size, v, val_w);
            let h = lines.len() as f32 * size * LINE + 3.0;
            self.ensure(h);
            let base = self.y + size;
            self.text_at(MARGIN_X, base, size, FontKind::Regular, SUBTEXT, *k);
            for (i, l) in lines.into_iter().enumerate() {
                self.text_at(MARGIN_X + key_w, base + i as f32 * size * LINE, size, FontKind::Regular, TEXT, l);
            }
            self.y += h;
        }
        self.y += 4.0;
    }

    pub fn table(&mut self, cols: &[Col], rows: &[Vec<Cell>]) {
        let width = self.content_width();
        let total: f32 = cols.iter().map(|c| c.weight).sum();
        let widths: Vec<f32> = cols.iter().map(|c| c.weight / total * width).collect();
        let font_of = |c: &Col, cell: &Cell| match (c.mono, cell.bold) {
            (_, true) => FontKind::Bold,
            (true, false) => FontKind::Mono,
            _ => FontKind::Regular,
        };
        let size = TABLE_SIZE;
        let lh = size * LINE;
        let layout_row = |cells: Vec<(&Col, &Cell)>, widths: &[f32]| -> Vec<Vec<String>> {
            cells
                .into_iter()
                .zip(widths)
                .map(|((c, cell), w)| wrap(font_of(c, cell), size, &cell.text, w - 2.0 * CELL_PAD))
                .collect()
        };
        let header: Vec<Cell> = cols.iter().map(|c| bold(c.title.clone())).collect();
        let header_lines = layout_row(cols.iter().zip(&header).collect(), &widths);
        let header_h = header_lines.iter().map(|l| l.len()).max().unwrap_or(1) as f32 * lh + 2.0 * CELL_PAD;

        let draw_row = |doc: &mut Doc, cells: &[Cell], lines: &[Vec<String>], h: f32, bg: Option<Rgb>, is_header: bool| {
            let y0 = doc.y;
            if let Some(bg) = bg {
                doc.push(Op::Rect { x: MARGIN_X, y: y0, w: width, h, fill: Some(bg), stroke: None });
            }
            let mut x = MARGIN_X;
            for ((c, cell), (w, ls)) in cols.iter().zip(cells).zip(widths.iter().zip(lines)) {
                let font = if is_header { FontKind::Bold } else { font_of(c, cell) };
                let color = if is_header { SUBTEXT } else { cell.color.unwrap_or(TEXT) };
                for (i, l) in ls.iter().enumerate() {
                    let tx = match c.align {
                        Align::Left => x + CELL_PAD,
                        Align::Right => x + w - CELL_PAD - text_width(font, size, l),
                    };
                    doc.text_at(tx, y0 + CELL_PAD + (i as f32 + 1.0) * lh - size * 0.3, size, font, color, l.clone());
                }
                x += w;
            }
            doc.push(Op::Line { x1: MARGIN_X, y1: y0 + h, x2: MARGIN_X + width, y2: y0 + h, color: RULE, width: 0.5 });
            doc.y += h;
        };

        self.ensure(header_h + 2.0 * lh + 2.0 * CELL_PAD);
        draw_row(self, &header, &header_lines, header_h, Some(HEAD_BG), true);
        for (i, row) in rows.iter().enumerate() {
            let lines = layout_row(cols.iter().zip(row).collect(), &widths);
            let h = lines.iter().map(|l| l.len()).max().unwrap_or(1) as f32 * lh + 2.0 * CELL_PAD;
            if self.remaining() < h {
                let landscape = self.pages.last().is_some_and(|p| p.width > p.height);
                self.new_page(landscape);
                draw_row(self, &header, &header_lines, header_h, Some(HEAD_BG), true);
            }
            draw_row(self, row, &lines, h, (i % 2 == 1).then_some(ZEBRA), false);
        }
        self.y += 8.0;
    }

    /// Place an SVG scaled to fit the rest of the page (and at most 1:1).
    pub fn svg(&mut self, svg: String, w: f32, h: f32, max_h: Option<f32>) {
        let avail_w = self.content_width();
        let avail_h = max_h.unwrap_or(self.remaining()).min(self.remaining());
        let scale = (avail_w / w).min(avail_h / h).min(1.0);
        let (dw, dh) = (w * scale, h * scale);
        let x = MARGIN_X + (avail_w - dw) / 2.0;
        self.push(Op::Rect { x: x - 0.5, y: self.y - 0.5, w: dw + 1.0, h: dh + 1.0, fill: None, stroke: Some((RULE, 0.7)) });
        self.push(Op::Svg { svg, x, y: self.y, w: dw, h: dh });
        self.y += dh + 8.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fonts_cover_report_symbols() {
        for k in [FontKind::Regular, FontKind::Bold, FontKind::Mono] {
            assert!(covers(k, "SW-01:ge3 ↔ PC → 10.0.0.1/24 · 2×10G ≤ 20:1 ≈ — – … “x” ⚠ ✓"), "{k:?}");
        }
        assert!(text_width(FontKind::Regular, 10.0, "Hello") > 15.0);
        assert!(text_width(FontKind::Bold, 10.0, "Hello") > text_width(FontKind::Regular, 10.0, "Hello"));
    }

    #[test]
    fn wrapping() {
        let lines = wrap(FontKind::Regular, 10.0, "the quick brown fox jumps over the lazy dog", 60.0);
        assert!(lines.len() > 2);
        assert!(lines.iter().all(|l| text_width(FontKind::Regular, 10.0, l) <= 60.0));
        let long = wrap(FontKind::Mono, 8.0, "2001:0db8:0000:0000:0000:0000:0000:0001", 40.0);
        assert!(long.len() > 1 && long.concat() == "2001:0db8:0000:0000:0000:0000:0000:0001");
        assert_eq!(wrap(FontKind::Regular, 10.0, "a\nb", 100.0), vec!["a", "b"]);
    }

    #[test]
    fn tables_break_pages_and_repeat_header() {
        let mut d = Doc::new();
        d.new_page(false);
        let rows: Vec<Vec<Cell>> = (0..200).map(|i| vec![Cell::from(format!("row {i}")), Cell::from("x")]).collect();
        d.table(&[col("A", 1.0), col("B", 1.0)], &rows);
        assert!(d.pages.len() >= 3);
        for p in &d.pages {
            let headers = p.ops.iter().filter(|o| matches!(o, Op::Text { text, font: FontKind::Bold, .. } if text == "A")).count();
            assert_eq!(headers, 1, "each page repeats the header once");
            for o in &p.ops {
                if let Op::Text { y, .. } = o {
                    assert!(*y <= p.height - BOTTOM + 1.0);
                }
            }
        }
    }
}
