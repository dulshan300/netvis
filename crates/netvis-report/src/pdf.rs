//! Write a laid-out [`Doc`] to PDF with krilla: fonts are embedded (subset),
//! diagrams stay vector, contents entries are links and sections become
//! bookmarks.

use krilla::Document;
use krilla::annotation::{Annotation, LinkAnnotation, Target};
use krilla::color::rgb;
use krilla::destination::{Destination, XyzDestination};
use krilla::geom::{PathBuilder, Point, Rect, Size, Transform};
use krilla::metadata::{DateTime, Metadata};
use krilla::outline::{Outline, OutlineNode};
use krilla::page::PageSettings;
use krilla::paint::{Fill, Stroke};
use krilla::surface::Surface;
use krilla::text::{Font, TextDirection};
use krilla_svg::{SurfaceExt, SvgSettings};

use crate::layout::{Doc, FontKind, Op, Rgb};

pub struct PdfInfo {
    pub title: String,
    pub author: String,
    pub date: (u16, u8, u8),
}

#[derive(Debug, thiserror::Error)]
pub enum PdfError {
    #[error("could not write the PDF: {0}")]
    Krilla(String),
    #[error("could not draw the diagram: {0}")]
    Svg(String),
}

fn color(c: Rgb) -> rgb::Color {
    rgb::Color::new(c[0], c[1], c[2])
}

fn fill(c: Rgb) -> Fill {
    Fill { paint: color(c).into(), ..Default::default() }
}

fn rect_path(x: f32, y: f32, w: f32, h: f32) -> Option<krilla::geom::Path> {
    let mut pb = PathBuilder::new();
    pb.push_rect(Rect::from_xywh(x, y, w.max(0.01), h.max(0.01))?);
    pb.finish()
}

struct Fonts {
    regular: Font,
    bold: Font,
    mono: Font,
}

impl Fonts {
    fn get(&self, k: FontKind) -> Font {
        match k {
            FontKind::Regular => self.regular.clone(),
            FontKind::Bold => self.bold.clone(),
            FontKind::Mono => self.mono.clone(),
        }
    }
}

fn svg_options() -> usvg::Options<'static> {
    let mut opt = usvg::Options { font_family: crate::diagram::SANS.into(), ..Default::default() };
    let db = opt.fontdb_mut();
    for k in [FontKind::Regular, FontKind::Bold, FontKind::Mono] {
        db.load_font_data(k.data().to_vec());
    }
    opt
}

fn draw_op(s: &mut Surface, fonts: &Fonts, svg_opt: &usvg::Options, op: &Op) -> Result<(), PdfError> {
    match op {
        Op::Text { x, y, size, font, color: c, text } => {
            s.set_fill(Some(fill(*c)));
            s.set_stroke(None);
            s.draw_text(Point::from_xy(*x, *y), fonts.get(*font), *size, text, false, TextDirection::Auto);
        }
        Op::Rect { x, y, w, h, fill: f, stroke } => {
            if let Some(path) = rect_path(*x, *y, *w, *h) {
                s.set_fill(f.map(fill));
                s.set_stroke(stroke.map(|(c, width)| Stroke { paint: color(c).into(), width, ..Default::default() }));
                s.draw_path(&path);
            }
        }
        Op::Line { x1, y1, x2, y2, color: c, width } => {
            let mut pb = PathBuilder::new();
            pb.move_to(*x1, *y1);
            pb.line_to(*x2, *y2);
            if let Some(path) = pb.finish() {
                s.set_fill(None);
                s.set_stroke(Some(Stroke { paint: color(*c).into(), width: *width, ..Default::default() }));
                s.draw_path(&path);
            }
        }
        Op::Svg { svg, x, y, w, h } => {
            let tree = usvg::Tree::from_str(svg, svg_opt).map_err(|e| PdfError::Svg(e.to_string()))?;
            let size = Size::from_wh(*w, *h).ok_or_else(|| PdfError::Svg("empty diagram".into()))?;
            s.push_transform(&Transform::from_translate(*x, *y));
            s.draw_svg(&tree, size, SvgSettings::default());
            s.pop();
        }
        Op::Link { .. } => {}
    }
    Ok(())
}

/// Running header and footer for content pages.
pub struct Furniture {
    pub left_header: String,
    pub right_header: String,
    pub left_footer: String,
}

pub fn write(doc: &Doc, info: &PdfInfo, furniture: &Furniture) -> Result<Vec<u8>, PdfError> {
    let mk = |k: FontKind| Font::new(k.data().into(), 0).ok_or_else(|| PdfError::Krilla("bundled font failed to load".into()));
    let fonts = Fonts { regular: mk(FontKind::Regular)?, bold: mk(FontKind::Bold)?, mono: mk(FontKind::Mono)? };
    let svg_opt = svg_options();
    let total = doc.pages.len();

    let mut pdf = Document::new();
    for (i, page) in doc.pages.iter().enumerate() {
        let settings = PageSettings::from_wh(page.width, page.height).ok_or_else(|| PdfError::Krilla("bad page size".into()))?;
        let mut pg = pdf.start_page_with(settings);
        {
            let mut s = pg.surface();
            for op in &page.ops {
                draw_op(&mut s, &fonts, &svg_opt, op)?;
            }
            if !page.plain {
                let (w, h) = (page.width, page.height);
                let x0 = crate::layout::MARGIN_X;
                let sub = crate::layout::SUBTEXT;
                let small = 7.5;
                let ops = [
                    Op::Text { x: x0, y: 34.0, size: small, font: FontKind::Bold, color: netvis_core::style::ACCENT, text: furniture.left_header.clone() },
                    Op::Text {
                        x: w - x0 - crate::layout::text_width(FontKind::Regular, small, &furniture.right_header),
                        y: 34.0,
                        size: small,
                        font: FontKind::Regular,
                        color: sub,
                        text: furniture.right_header.clone(),
                    },
                    Op::Line { x1: x0, y1: 41.0, x2: w - x0, y2: 41.0, color: crate::layout::RULE, width: 0.5 },
                    Op::Line { x1: x0, y1: h - 38.0, x2: w - x0, y2: h - 38.0, color: crate::layout::RULE, width: 0.5 },
                    Op::Text { x: x0, y: h - 26.0, size: small, font: FontKind::Regular, color: sub, text: furniture.left_footer.clone() },
                ];
                for op in &ops {
                    draw_op(&mut s, &fonts, &svg_opt, op)?;
                }
                let pn = format!("Page {} of {total}", i + 1);
                let x = w - x0 - crate::layout::text_width(FontKind::Regular, small, &pn);
                draw_op(&mut s, &fonts, &svg_opt, &Op::Text { x, y: h - 26.0, size: small, font: FontKind::Regular, color: sub, text: pn })?;
            }
            s.finish();
        }
        for op in &page.ops {
            if let Op::Link { x, y, w, h, page: target } = op
                && let Some(rect) = Rect::from_xywh(*x, *y, *w, *h)
            {
                let dest = XyzDestination::new(*target, Point::from_xy(0.0, 0.0));
                pg.add_annotation(Annotation::new_link(LinkAnnotation::new(rect, Target::Destination(Destination::Xyz(dest))), None));
            }
        }
        pg.finish();
    }

    // Bookmarks: sections with their subsections.
    let mut outline = Outline::new();
    let mut current: Option<OutlineNode> = None;
    for e in &doc.toc {
        let node = OutlineNode::new(e.title.clone(), XyzDestination::new(e.page, Point::from_xy(0.0, e.y)));
        if e.level == 0 {
            if let Some(n) = current.take() {
                outline.push_child(n);
            }
            current = Some(node.with_open(false));
        } else if let Some(parent) = current.as_mut() {
            parent.push_child(node);
        } else {
            outline.push_child(node);
        }
    }
    if let Some(n) = current {
        outline.push_child(n);
    }
    pdf.set_outline(outline);

    let (y, m, d) = info.date;
    let mut meta = Metadata::new()
        .title(info.title.clone())
        .creator("NetVis".into())
        .producer("NetVis (krilla)".into())
        .creation_date(DateTime::new(y).month(m).day(d));
    if !info.author.is_empty() {
        meta = meta.authors(vec![info.author.clone()]);
    }
    pdf.set_metadata(meta);
    pdf.finish().map_err(|e| PdfError::Krilla(format!("{e:?}")))
}

#[cfg(test)]
mod tests {
    #[test]
    fn diagram_font_resolves() {
        let opt = super::svg_options();
        let fams: Vec<String> = opt.fontdb.faces().flat_map(|f| f.families.iter().map(|(n, _)| n.clone())).collect();
        assert!(fams.iter().any(|f| f == crate::diagram::SANS), "{fams:?}");
    }
}
