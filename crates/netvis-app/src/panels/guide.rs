//! The user guide: Markdown pages from `docs/`, built into the app and shown
//! in the Guide tab (Help → User guide, F1).
//!
//! The renderer covers the Markdown the guide uses: headings, paragraphs,
//! bullet and numbered lists (nested by two spaces), tables, fenced code,
//! `> ` tips, **bold**, *italic*, `code` and links. A link to another page
//! (`[Patching](patching.md)`) opens that page; web links open the browser.

use egui::{RichText, Ui};

use crate::icons;
use crate::state::State;

pub struct Page {
    pub key: &'static str,
    pub title: &'static str,
    pub group: &'static str,
    pub text: &'static str,
}

macro_rules! page {
    ($key:literal, $title:literal, $group:literal) => {
        Page { key: $key, title: $title, group: $group, text: include_str!(concat!("../../../../docs/", $key, ".md")) }
    };
}

pub const PAGES: &[Page] = &[
    page!("getting-started", "Getting started", "Start here"),
    page!("templates", "Templates and samples", "Start here"),
    page!("canvas", "Drawing the network", "Designing"),
    page!("devices", "Devices, ports and interfaces", "Designing"),
    page!("ip-planning", "VLANs, subnets and Wi-Fi", "Designing"),
    page!("physical", "Cables, optics, PoE and racks", "Designing"),
    page!("patching", "Patch panels and outlets", "Designing"),
    page!("design-checks", "Design checks and fixes", "Checking and sharing"),
    page!("reports", "Reports and exports", "Checking and sharing"),
    page!("it-tools", "IT tools", "Checking and sharing"),
    page!("tutorial-shopping-mall", "Tutorial: shopping mall", "Tutorials"),
    page!("shortcuts", "Keyboard and mouse", "Reference"),
    page!("files", "Files, autosave and settings", "Reference"),
];

#[derive(Debug, Default)]
pub struct GuideState {
    pub page: usize,
    pub search: String,
    /// Scroll the page back to the top on the next frame.
    pub jump: bool,
}

impl GuideState {
    pub fn open(&mut self, key: &str) {
        if let Some(i) = PAGES.iter().position(|p| p.key == key) {
            self.page = i;
            self.jump = true;
        }
    }
}

pub fn show(ui: &mut Ui, st: &mut State) {
    let g = &mut st.guide;
    egui::Panel::left("guide.nav").resizable(false).exact_size(220.0).frame(egui::Frame::NONE).show(ui, |ui| {
        ui.add(
            egui::TextEdit::singleline(&mut g.search).hint_text(format!("{} Search the guide", icons::MAGNIFYING_GLASS)).desired_width(f32::INFINITY),
        );
        ui.add_space(6.0);
        let q = g.search.trim().to_lowercase();
        egui::ScrollArea::vertical().show(ui, |ui| {
            let mut group = "";
            for (i, p) in PAGES.iter().enumerate() {
                let hits = if q.is_empty() { 1 } else { p.text.to_lowercase().matches(&q).count() + p.title.to_lowercase().matches(&q).count() };
                if hits == 0 {
                    continue;
                }
                if p.group != group {
                    group = p.group;
                    ui.add_space(4.0);
                    ui.label(RichText::new(group.to_uppercase()).small().weak());
                }
                let label = if q.is_empty() { p.title.to_owned() } else { format!("{}  ({hits})", p.title) };
                if ui.selectable_label(g.page == i, label).clicked() {
                    g.page = i;
                    g.jump = true;
                }
            }
        });
    });

    let page = &PAGES[g.page.min(PAGES.len() - 1)];
    let mut scroll = egui::ScrollArea::vertical().id_salt(("guide.page", page.key)).auto_shrink(false);
    if std::mem::take(&mut g.jump) {
        scroll = scroll.vertical_scroll_offset(0.0);
    }
    let search = g.search.trim().to_owned();
    let mut go: Option<String> = None;
    scroll.show(ui, |ui| {
        ui.set_max_width(820.0);
        ui.add_space(4.0);
        render(ui, page.text, &search, &mut go);
        ui.add_space(12.0);
        // Previous / next.
        ui.separator();
        ui.horizontal(|ui| {
            let i = PAGES.iter().position(|p| p.key == page.key).unwrap_or(0);
            if i > 0 && ui.button(format!("{} {}", icons::ARROW_LEFT, PAGES[i - 1].title)).clicked() {
                go = Some(PAGES[i - 1].key.into());
            }
            if i + 1 < PAGES.len() && ui.button(format!("{} {}", PAGES[i + 1].title, icons::ARROW_RIGHT)).clicked() {
                go = Some(PAGES[i + 1].key.into());
            }
        });
    });
    if let Some(target) = go {
        if target.starts_with("http") {
            ui.ctx().open_url(egui::OpenUrl::new_tab(target));
        } else {
            st.guide.open(&target);
        }
    }
}

// ---- Markdown ----------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Block {
    Heading(u8, String),
    Para(String),
    /// Indent level, number (for numbered lists), text.
    Item(usize, Option<String>, String),
    Tip(String),
    Code(String),
    Table(Vec<Vec<String>>),
    Rule,
}

fn parse(text: &str) -> Vec<Block> {
    let mut out = Vec::new();
    let mut para = String::new();
    let mut lines = text.lines().peekable();
    let flush = |para: &mut String, out: &mut Vec<Block>| {
        if !para.trim().is_empty() {
            out.push(Block::Para(para.trim().to_owned()));
        }
        para.clear();
    };
    while let Some(line) = lines.next() {
        let t = line.trim_start();
        let indent = (line.len() - t.len()) / 2;
        if t.starts_with("```") {
            flush(&mut para, &mut out);
            let mut code = String::new();
            for l in lines.by_ref() {
                if l.trim_start().starts_with("```") {
                    break;
                }
                code.push_str(l);
                code.push('\n');
            }
            out.push(Block::Code(code.trim_end().to_owned()));
        } else if t.is_empty() {
            flush(&mut para, &mut out);
        } else if let Some(h) = t.strip_prefix("### ") {
            flush(&mut para, &mut out);
            out.push(Block::Heading(3, h.into()));
        } else if let Some(h) = t.strip_prefix("## ") {
            flush(&mut para, &mut out);
            out.push(Block::Heading(2, h.into()));
        } else if let Some(h) = t.strip_prefix("# ") {
            flush(&mut para, &mut out);
            out.push(Block::Heading(1, h.into()));
        } else if t == "---" {
            flush(&mut para, &mut out);
            out.push(Block::Rule);
        } else if let Some(q) = t.strip_prefix("> ") {
            flush(&mut para, &mut out);
            let mut tip = q.to_owned();
            while let Some(next) = lines.peek().and_then(|l| l.trim_start().strip_prefix("> ")) {
                tip.push(' ');
                tip.push_str(next);
                lines.next();
            }
            out.push(Block::Tip(tip));
        } else if t.starts_with('|') {
            flush(&mut para, &mut out);
            let mut rows = vec![cells(t)];
            while let Some(next) = lines.peek().filter(|l| l.trim_start().starts_with('|')) {
                let r = cells(next.trim_start());
                if !r.iter().all(|c| c.chars().all(|ch| matches!(ch, '-' | ':' | ' '))) {
                    rows.push(r);
                }
                lines.next();
            }
            out.push(Block::Table(rows));
        } else if let Some(rest) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) {
            flush(&mut para, &mut out);
            out.push(Block::Item(indent, None, rest.into()));
        } else if let Some((n, rest)) = t.split_once(". ").filter(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit())) {
            flush(&mut para, &mut out);
            out.push(Block::Item(indent, Some(n.into()), rest.into()));
        } else if let Some(Block::Item(_, _, text)) = out.last_mut().filter(|_| para.is_empty() && indent > 0) {
            // Continuation of a list item.
            text.push(' ');
            text.push_str(t);
        } else {
            para.push_str(t);
            para.push(' ');
        }
    }
    flush(&mut para, &mut out);
    out
}

fn cells(row: &str) -> Vec<String> {
    let row = row.trim().trim_start_matches('|').trim_end_matches('|');
    row.split('|').map(|c| c.trim().to_owned()).collect()
}

#[derive(Debug, Clone, PartialEq)]
enum Span {
    Text { text: String, bold: bool, italic: bool },
    Code(String),
    Link { text: String, target: String },
}

/// Split inline Markdown into styled runs.
fn spans(s: &str) -> Vec<Span> {
    let mut out = Vec::new();
    let (mut bold, mut italic) = (false, false);
    let mut buf = String::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    let push = |buf: &mut String, out: &mut Vec<Span>, bold, italic| {
        if !buf.is_empty() {
            out.push(Span::Text { text: std::mem::take(buf), bold, italic });
        }
    };
    while i < chars.len() {
        let c = chars[i];
        if c == '`'
            && let Some(end) = chars[i + 1..].iter().position(|x| *x == '`')
        {
            push(&mut buf, &mut out, bold, italic);
            out.push(Span::Code(chars[i + 1..i + 1 + end].iter().collect()));
            i += end + 2;
        } else if c == '*' && chars.get(i + 1) == Some(&'*') {
            push(&mut buf, &mut out, bold, italic);
            bold = !bold;
            i += 2;
        } else if c == '*' && (italic || chars.get(i + 1).is_some_and(|n| !n.is_whitespace())) {
            push(&mut buf, &mut out, bold, italic);
            italic = !italic;
            i += 1;
        } else if c == '['
            && let Some(close) = chars[i..].iter().position(|x| *x == ']').map(|p| p + i)
            && chars.get(close + 1) == Some(&'(')
            && let Some(end) = chars[close..].iter().position(|x| *x == ')').map(|p| p + close)
        {
            push(&mut buf, &mut out, bold, italic);
            let text: String = chars[i + 1..close].iter().collect();
            let target: String = chars[close + 2..end].iter().collect();
            out.push(Span::Link { text, target });
            i = end + 1;
        } else {
            buf.push(c);
            i += 1;
        }
    }
    push(&mut buf, &mut out, bold, italic);
    out
}

/// A guide page key for a relative `.md` link, or the URL itself.
fn resolve(target: &str) -> String {
    if target.starts_with("http") {
        return target.to_owned();
    }
    let file = target.split('#').next().unwrap_or("").rsplit('/').next().unwrap_or("");
    file.trim_end_matches(".md").to_owned()
}

fn inline(ui: &mut Ui, s: &str, size: f32, search: &str, go: &mut Option<String>) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for span in spans(s) {
            match span {
                Span::Text { text, bold, italic } => {
                    let mut rt = RichText::new(text.clone()).size(size);
                    if bold {
                        rt = rt.strong();
                    }
                    if italic {
                        rt = rt.italics();
                    }
                    if !search.is_empty() && text.to_lowercase().contains(&search.to_lowercase()) {
                        rt = rt.background_color(ui.visuals().selection.bg_fill.gamma_multiply(0.35));
                    }
                    ui.label(rt);
                }
                Span::Code(c) => {
                    ui.label(RichText::new(c).code().size(size - 1.0));
                }
                Span::Link { text, target } => {
                    let r = ui.link(RichText::new(text).size(size));
                    let dest = resolve(&target);
                    if r.on_hover_text(&dest).clicked() {
                        *go = Some(dest);
                    }
                }
            }
        }
    });
}

fn render(ui: &mut Ui, text: &str, search: &str, go: &mut Option<String>) {
    const BODY: f32 = 14.0;
    for block in parse(text) {
        match block {
            Block::Heading(level, h) => {
                let (size, space) = match level {
                    1 => (24.0, 2.0),
                    2 => (18.0, 12.0),
                    _ => (15.5, 8.0),
                };
                ui.add_space(space);
                ui.label(RichText::new(h).size(size).strong());
                if level == 1 {
                    ui.add_space(2.0);
                }
            }
            Block::Para(p) => {
                inline(ui, &p, BODY, search, go);
                ui.add_space(6.0);
            }
            Block::Item(indent, num, t) => {
                ui.horizontal_top(|ui| {
                    ui.add_space(8.0 + indent as f32 * 18.0);
                    let marker = num.map(|n| format!("{n}.")).unwrap_or_else(|| if indent == 0 { "•".into() } else { "◦".into() });
                    ui.add_sized([18.0, 18.0], egui::Label::new(RichText::new(marker).size(BODY).weak()));
                    ui.vertical(|ui| inline(ui, &t, BODY, search, go));
                });
                ui.add_space(2.0);
            }
            Block::Tip(t) => {
                egui::Frame::new()
                    .fill(ui.visuals().selection.bg_fill.gamma_multiply(0.15))
                    .stroke(egui::Stroke::new(1.0, ui.visuals().selection.bg_fill.gamma_multiply(0.6)))
                    .corner_radius(4.0)
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        ui.horizontal_top(|ui| {
                            ui.label(RichText::new(icons::LIGHTBULB).size(16.0));
                            ui.vertical(|ui| inline(ui, &t, BODY - 0.5, search, go));
                        });
                    });
                ui.add_space(6.0);
            }
            Block::Code(c) => {
                egui::Frame::new().fill(ui.visuals().extreme_bg_color).corner_radius(4.0).inner_margin(8.0).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(c).monospace().size(12.5));
                });
                ui.add_space(6.0);
            }
            Block::Table(rows) => {
                let n = rows.iter().map(Vec::len).max().unwrap_or(1);
                egui::Frame::new().stroke(ui.visuals().widgets.noninteractive.bg_stroke).corner_radius(4.0).inner_margin(6.0).show(ui, |ui| {
                    egui::Grid::new(ui.next_auto_id()).num_columns(n).striped(true).spacing([14.0, 5.0]).show(ui, |ui| {
                        for (r, row) in rows.iter().enumerate() {
                            for c in 0..n {
                                let cell = row.get(c).map(String::as_str).unwrap_or("");
                                if r == 0 {
                                    ui.label(RichText::new(cell.replace("**", "")).strong().size(BODY - 1.0));
                                } else {
                                    ui.scope(|ui| {
                                        ui.set_max_width(if c + 1 == n { 460.0 } else { 260.0 });
                                        inline(ui, cell, BODY - 1.0, search, go);
                                    });
                                }
                            }
                            ui.end_row();
                        }
                    });
                });
                ui.add_space(8.0);
            }
            Block::Rule => {
                ui.separator();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_blocks_and_spans() {
        let b = parse(
            "# Title\n\nSome **bold** text\nwrapped.\n\n- one\n  - two\n1. first\n\n> tip\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\n```\ncode\n```\n",
        );
        assert_eq!(b[0], Block::Heading(1, "Title".into()));
        assert_eq!(b[1], Block::Para("Some **bold** text wrapped.".into()));
        assert_eq!(b[2], Block::Item(0, None, "one".into()));
        assert_eq!(b[3], Block::Item(1, None, "two".into()));
        assert_eq!(b[4], Block::Item(0, Some("1".into()), "first".into()));
        assert_eq!(b[5], Block::Tip("tip".into()));
        assert_eq!(b[6], Block::Table(vec![vec!["A".into(), "B".into()], vec!["1".into(), "2".into()]]));
        assert_eq!(b[7], Block::Code("code".into()));
        let s = spans("a **b** `c` [d](patching.md#x) *e*");
        assert!(s.contains(&Span::Text { text: "b".into(), bold: true, italic: false }));
        assert!(s.contains(&Span::Code("c".into())));
        assert!(s.contains(&Span::Link { text: "d".into(), target: "patching.md#x".into() }));
        assert!(s.contains(&Span::Text { text: "e".into(), bold: false, italic: true }));
        assert_eq!(resolve("patching.md#x"), "patching");
        assert_eq!(resolve("../docs/canvas.md"), "canvas");
    }

    /// Every page link points at a page that exists.
    #[test]
    fn links_resolve() {
        for p in PAGES {
            for block in parse(p.text) {
                let text = match block {
                    Block::Para(t) | Block::Item(_, _, t) | Block::Tip(t) => t,
                    Block::Table(rows) => rows.concat().join(" "),
                    _ => continue,
                };
                for s in spans(&text) {
                    if let Span::Link { target, .. } = s {
                        let r = resolve(&target);
                        assert!(r.starts_with("http") || PAGES.iter().any(|q| q.key == r), "{}: broken link {target}", p.key);
                    }
                }
            }
        }
    }
}
