//! The topology canvas: drawing, hit-testing and direct manipulation.

use egui::{
    Align2, Color32, CornerRadius, CursorIcon, FontId, PointerButton, Pos2, Rect, Sense, Shape, Stroke,
    StrokeKind, Ui, Vec2, pos2, vec2,
};
use netvis_core::device::layout;
use netvis_core::{
    Area, AreaId, AreaKind, Device, DeviceId, DeviceKind, Edit, Endpoint, Link, Pos, PoeRole, Project, SwitchportMode,
};

use crate::icons;
use crate::state::{Item, State};
use crate::theme::{self, Palette};

pub const GRID: f32 = 20.0;
const AREA_HEADER: f32 = 26.0;
const AREA_HANDLE: f32 = 14.0;
const MIN_ZOOM: f32 = 0.1;
const MAX_ZOOM: f32 = 4.0;
/// Below this zoom, port markers are hidden and cannot be targeted.
const PORT_DETAIL_ZOOM: f32 = 0.45;

/// Dragged from the toolbox onto the canvas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PaletteItem {
    Device(DeviceKind),
    Area(AreaKind),
}

#[derive(Debug, Clone, Copy)]
pub struct Camera {
    /// World coordinate shown at the canvas' top-left corner.
    pub pan: Vec2,
    pub zoom: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self { pan: vec2(-40.0, -40.0), zoom: 1.0 }
    }
}

impl Camera {
    fn to_screen(self, origin: Pos2, p: Pos) -> Pos2 {
        origin + (vec2(p.x, p.y) - self.pan) * self.zoom
    }

    fn to_world(self, origin: Pos2, s: Pos2) -> Pos {
        let v = (s - origin) / self.zoom + self.pan;
        Pos::new(v.x, v.y)
    }

    fn rect_to_screen(self, origin: Pos2, r: netvis_core::Rect) -> Rect {
        Rect::from_min_max(self.to_screen(origin, r.min), self.to_screen(origin, r.max()))
    }

    fn zoom_about(&mut self, origin: Pos2, anchor: Pos2, factor: f32) {
        let w = self.to_world(origin, anchor);
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        self.pan = vec2(w.x, w.y) - (anchor - origin) / self.zoom;
    }

    fn center_on(&mut self, viewport: Rect, world: Pos) {
        self.pan = vec2(world.x, world.y) - viewport.size() / (2.0 * self.zoom);
    }

    fn fit(&mut self, viewport: Rect, bounds: netvis_core::Rect) {
        let margin = 60.0;
        let avail = (viewport.size() - Vec2::splat(2.0 * margin)).max(Vec2::splat(50.0));
        let size = vec2(bounds.size.x.max(1.0), bounds.size.y.max(1.0));
        self.zoom = (avail.x / size.x).min(avail.y / size.y).clamp(MIN_ZOOM, 1.5);
        let c = bounds.min.offset(bounds.size.x / 2.0, bounds.size.y / 2.0);
        self.center_on(viewport, c);
    }
}

#[derive(Debug, Clone, Default)]
enum Drag {
    #[default]
    None,
    Pan,
    Move {
        origin: Pos,
        devices: Vec<(DeviceId, Pos)>,
        areas: Vec<(AreaId, netvis_core::Rect)>,
    },
    Resize {
        area: AreaId,
        origin: Pos,
        start: netvis_core::Rect,
    },
    Marquee {
        start: Pos2,
        additive: bool,
    },
    Link {
        from: Endpoint,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Hit {
    None,
    Port(Endpoint),
    Device(DeviceId),
    AreaHeader(AreaId),
    AreaHandle(AreaId),
    Link(netvis_core::LinkId),
}

pub struct CanvasState {
    pub camera: Camera,
    drag: Drag,
    fit_requested: bool,
    focus_requested: Option<Pos>,
    viewport: Rect,
    pub cursor_world: Option<Pos>,
}

impl Default for CanvasState {
    fn default() -> Self {
        Self {
            camera: Camera::default(),
            drag: Drag::None,
            fit_requested: false,
            focus_requested: None,
            viewport: Rect::NOTHING,
            cursor_world: None,
        }
    }
}

impl CanvasState {
    pub fn request_fit(&mut self) {
        self.fit_requested = true;
    }

    pub fn focus(&mut self, world: Pos) {
        self.focus_requested = Some(world);
    }

    pub fn zoom_step(&mut self, factor: f32) {
        let c = self.viewport.center();
        self.camera.zoom_about(self.viewport.min, c, factor);
    }

    pub fn reset_zoom(&mut self) {
        let c = self.viewport.center();
        let f = 1.0 / self.camera.zoom;
        self.camera.zoom_about(self.viewport.min, c, f);
    }

    /// World coordinate at the centre of the visible canvas.
    pub fn view_center(&self) -> Pos {
        if self.viewport.width() <= 0.0 {
            return Pos::new(200.0, 200.0);
        }
        self.camera.to_world(self.viewport.min, self.viewport.center())
    }

    pub fn is_dragging(&self) -> bool {
        !matches!(self.drag, Drag::None)
    }
}

// ---- hit testing ---------------------------------------------------------

fn hit_test(p: &Project, w: Pos, zoom: f32) -> Hit {
    for d in p.devices.iter().rev() {
        if !d.rect().contains(w) {
            continue;
        }
        if zoom >= PORT_DETAIL_ZOOM {
            let half = layout::PORT_PITCH / 2.0;
            for (i, port) in d.ports.iter().enumerate() {
                let c = d.port_center(i);
                if (c.x - w.x).abs() <= half && (c.y - w.y).abs() <= half {
                    return Hit::Port(Endpoint { device: d.id, port: port.id });
                }
            }
        }
        return Hit::Device(d.id);
    }

    let tol = 6.0 / zoom;
    for l in p.links.iter().rev() {
        if let Some((a, b)) = link_points(p, l)
            && dist_to_segment(w, a, b) <= tol
        {
            return Hit::Link(l.id);
        }
    }

    for a in p.areas.iter().rev() {
        let r = a.rect;
        let max = r.max();
        if w.x >= max.x - AREA_HANDLE && w.x <= max.x && w.y >= max.y - AREA_HANDLE && w.y <= max.y {
            return Hit::AreaHandle(a.id);
        }
        if w.x >= r.min.x && w.x <= max.x && w.y >= r.min.y && w.y <= r.min.y + AREA_HEADER {
            return Hit::AreaHeader(a.id);
        }
    }
    Hit::None
}

fn link_points(p: &Project, l: &Link) -> Option<(Pos, Pos)> {
    let a = p.device(l.a.device)?.port_center_by_id(l.a.port)?;
    let b = p.device(l.b.device)?.port_center_by_id(l.b.port)?;
    Some((a, b))
}

fn dist_to_segment(p: Pos, a: Pos, b: Pos) -> f32 {
    let (abx, aby) = (b.x - a.x, b.y - a.y);
    let len2 = abx * abx + aby * aby;
    let t = if len2 == 0.0 { 0.0 } else { (((p.x - a.x) * abx + (p.y - a.y) * aby) / len2).clamp(0.0, 1.0) };
    let (cx, cy) = (a.x + t * abx, a.y + t * aby);
    ((p.x - cx).powi(2) + (p.y - cy).powi(2)).sqrt()
}

fn item_of(hit: Hit) -> Option<Item> {
    match hit {
        Hit::Port(ep) => Some(Item::Device(ep.device)),
        Hit::Device(id) => Some(Item::Device(id)),
        Hit::AreaHeader(id) | Hit::AreaHandle(id) => Some(Item::Area(id)),
        Hit::Link(id) => Some(Item::Link(id)),
        Hit::None => None,
    }
}

// ---- main entry ----------------------------------------------------------

pub fn show(ui: &mut Ui, st: &mut State) {
    let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
    let origin = rect.min;
    st.canvas.viewport = rect;

    if st.canvas.fit_requested && rect.width() > 0.0 {
        st.canvas.fit_requested = false;
        match st.doc.project.bounds() {
            Some(b) => st.canvas.camera.fit(rect, b),
            None => st.canvas.camera = Camera::default(),
        }
    }
    if let Some(w) = st.canvas.focus_requested.take() {
        st.canvas.camera.center_on(rect, w);
    }

    let pal = Palette::new(ui.visuals().dark_mode);
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, pal.canvas_bg);
    if st.settings.show_grid {
        draw_grid(&painter, rect, &st.canvas.camera, &pal);
    }

    handle_zoom_and_pan(ui, &response, st);

    let cam = st.canvas.camera;
    let pointer = response.hover_pos();
    st.canvas.cursor_world = pointer.map(|p| cam.to_world(origin, p));
    let hit = st.canvas.cursor_world.map_or(Hit::None, |w| hit_test(&st.doc.project, w, cam.zoom));
    let shift = ui.input(|i| i.modifiers.shift);
    let space = ui.input(|i| i.key_down(egui::Key::Space));

    handle_primary_drag(ui, &response, st, space, shift);
    handle_clicks(&response, st, hit, shift);
    handle_drop(ui, &response, st);

    // ---- paint ----
    let cam = st.canvas.camera;
    let p = &st.doc.project;
    for a in &p.areas {
        draw_area(&painter, &cam, origin, a, st.sel.contains(Item::Area(a.id)), &pal);
    }
    for l in &p.links {
        draw_link(&painter, &cam, origin, p, l, st.sel.contains(Item::Link(l.id)), hit == Hit::Link(l.id), &pal);
    }
    if let (Drag::Link { from }, Some(ptr)) = (&st.canvas.drag, pointer)
        && let Some(a) = p.device(from.device).and_then(|d| d.port_center_by_id(from.port))
    {
        let stroke = Stroke::new(2.0, pal.accent);
        painter.extend(Shape::dashed_line(&[cam.to_screen(origin, a), ptr], stroke, 6.0, 4.0));
    }
    let hover_port = match hit {
        Hit::Port(ep) => Some(ep),
        _ => None,
    };
    for d in &p.devices {
        let selected = st.sel.contains(Item::Device(d.id));
        draw_device(&painter, &cam, origin, p, d, selected, hover_port, st.settings.show_addresses, &pal);
    }
    draw_issue_badges(&painter, &cam, origin, p, &st.issues);
    if let (Drag::Marquee { start, .. }, Some(ptr)) = (&st.canvas.drag, pointer) {
        let r = Rect::from_two_pos(*start, ptr);
        painter.rect_filled(r, 2.0, pal.accent.gamma_multiply(0.08));
        painter.rect_stroke(r, 2.0, Stroke::new(1.0, pal.accent), StrokeKind::Inside);
    }

    if p.devices.is_empty() && p.areas.is_empty() {
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            "Drag devices from the Toolbox to start designing",
            FontId::proportional(15.0),
            pal.subtext,
        );
    }

    // ---- cursor & tooltips ----
    if !st.canvas.is_dragging() {
        match hit {
            Hit::Port(ep) => {
                ui.ctx().set_cursor_icon(CursorIcon::Crosshair);
                if let Some(text) = port_tooltip(p, ep) {
                    response.clone().on_hover_text_at_pointer(text);
                }
            }
            Hit::Device(_) | Hit::AreaHeader(_) => ui.ctx().set_cursor_icon(CursorIcon::Grab),
            Hit::AreaHandle(_) => ui.ctx().set_cursor_icon(CursorIcon::ResizeNwSe),
            Hit::Link(_) => ui.ctx().set_cursor_icon(CursorIcon::PointingHand),
            Hit::None if space => ui.ctx().set_cursor_icon(CursorIcon::Grab),
            Hit::None => {}
        }
    } else if matches!(st.canvas.drag, Drag::Pan | Drag::Move { .. }) {
        ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
    }

    context_menu(&response, st);

    if st.settings.show_minimap {
        minimap(ui, rect, st, &pal);
    }
}

fn handle_zoom_and_pan(ui: &Ui, response: &egui::Response, st: &mut State) {
    let origin = response.rect.min;
    if response.hovered() {
        let (scroll, zoom, ptr) = ui.input(|i| (i.smooth_scroll_delta, i.zoom_delta(), i.pointer.hover_pos()));
        if let Some(ptr) = ptr {
            let mut factor = zoom;
            if scroll.y != 0.0 {
                factor *= (scroll.y * 0.0018).exp();
            }
            if factor != 1.0 {
                st.canvas.camera.zoom_about(origin, ptr, factor);
            }
        }
    }

    let space = ui.input(|i| i.key_down(egui::Key::Space));
    let panning = response.dragged_by(PointerButton::Middle)
        || response.dragged_by(PointerButton::Secondary)
        || (space && response.dragged_by(PointerButton::Primary));
    if panning {
        if !matches!(st.canvas.drag, Drag::Pan) && !st.canvas.is_dragging() {
            st.canvas.drag = Drag::Pan;
        }
        if matches!(st.canvas.drag, Drag::Pan) {
            st.canvas.camera.pan -= response.drag_delta() / st.canvas.camera.zoom;
        }
    } else if matches!(st.canvas.drag, Drag::Pan) {
        st.canvas.drag = Drag::None;
    }
}

fn handle_primary_drag(ui: &Ui, response: &egui::Response, st: &mut State, space: bool, shift: bool) {
    let origin = response.rect.min;
    let cam = st.canvas.camera;
    let snap = st.settings.snap_to_grid;

    if response.drag_started_by(PointerButton::Primary) && !space {
        let Some(press) = ui.input(|i| i.pointer.press_origin()) else { return };
        let w = cam.to_world(origin, press);
        let hit = hit_test(&st.doc.project, w, cam.zoom);
        st.doc.history.break_coalescing();
        st.canvas.drag = match hit {
            Hit::Port(ep) => Drag::Link { from: ep },
            Hit::Device(_) | Hit::AreaHeader(_) => {
                let item = item_of(hit).expect("hit has item");
                if !st.sel.contains(item) {
                    st.sel.click(item, shift);
                }
                begin_move(&st.doc.project, &st.sel, w)
            }
            Hit::AreaHandle(id) => {
                st.sel.set_single(Item::Area(id));
                let start = st.doc.project.area(id).map(|a| a.rect).unwrap_or_default();
                Drag::Resize { area: id, origin: w, start }
            }
            Hit::Link(_) | Hit::None => Drag::Marquee { start: press, additive: shift },
        };
    }

    let Some(ptr) = ui.input(|i| i.pointer.interact_pos()) else { return };
    let w = cam.to_world(origin, ptr);

    if response.dragged_by(PointerButton::Primary) {
        match &st.canvas.drag {
            Drag::Move { origin: o, devices, areas } => {
                let (dx, dy) = (w.x - o.x, w.y - o.y);
                let place = |p: Pos| {
                    let n = p.offset(dx, dy);
                    if snap { n.snapped(GRID) } else { n }
                };
                for (id, start) in devices {
                    if let Some(d) = st.doc.project.device_mut(*id) {
                        d.pos = place(*start);
                    }
                }
                for (id, start) in areas {
                    if let Some(a) = st.doc.project.area_mut(*id) {
                        a.rect.min = place(start.min);
                    }
                }
            }
            Drag::Resize { area, origin: o, start } => {
                let mut w2 = start.size.x + (w.x - o.x);
                let mut h2 = start.size.y + (w.y - o.y);
                if snap {
                    w2 = (w2 / GRID).round() * GRID;
                    h2 = (h2 / GRID).round() * GRID;
                }
                if let Some(a) = st.doc.project.area_mut(*area) {
                    a.rect.size = Pos::new(w2.max(Area::MIN_SIZE), h2.max(Area::MIN_SIZE));
                }
            }
            _ => {}
        }
    }

    if response.drag_stopped() {
        let drag = std::mem::take(&mut st.canvas.drag);
        match drag {
            Drag::Move { devices, areas, .. } => finish_move(st, &devices, &areas),
            Drag::Resize { area, start, .. } => {
                if let Some(a) = st.doc.project.area(area) {
                    let mut before = a.clone();
                    before.rect = start;
                    let edit = Edit::Area { id: area, before: Some(before), after: Some(a.clone()) };
                    st.record_applied(edit, "Resize area");
                }
            }
            Drag::Marquee { start, additive } => {
                let r = Rect::from_two_pos(start, ptr);
                let (a, b) = (cam.to_world(origin, r.min), cam.to_world(origin, r.max));
                let wr = netvis_core::Rect::new(a.x, a.y, b.x - a.x, b.y - a.y);
                if !additive {
                    st.sel.clear();
                }
                let p = &st.doc.project;
                for d in &p.devices {
                    if wr.contains_rect(&d.rect()) {
                        st.sel.add(Item::Device(d.id));
                    }
                }
                for ar in &p.areas {
                    if wr.contains_rect(&ar.rect) {
                        st.sel.add(Item::Area(ar.id));
                    }
                }
                for l in &p.links {
                    if let Some((a, b)) = link_points(p, l)
                        && wr.contains(a)
                        && wr.contains(b)
                    {
                        st.sel.add(Item::Link(l.id));
                    }
                }
            }
            Drag::Link { from } => finish_link(st, from, w),
            Drag::Pan | Drag::None => {}
        }
    }
}

/// Collect everything that moves together: the selection plus the contents of
/// any selected area.
fn begin_move(p: &Project, sel: &crate::state::Selection, origin: Pos) -> Drag {
    let mut devices: Vec<(DeviceId, Pos)> =
        p.devices.iter().filter(|d| sel.devices.contains(&d.id)).map(|d| (d.id, d.pos)).collect();
    let mut areas: Vec<(AreaId, netvis_core::Rect)> =
        p.areas.iter().filter(|a| sel.areas.contains(&a.id)).map(|a| (a.id, a.rect)).collect();

    let selected_rects: Vec<_> = areas.iter().map(|(_, r)| *r).collect();
    for r in &selected_rects {
        for d in &p.devices {
            let dr = d.rect();
            let c = dr.min.offset(dr.size.x / 2.0, dr.size.y / 2.0);
            if r.contains(c) && !devices.iter().any(|(id, _)| *id == d.id) {
                devices.push((d.id, d.pos));
            }
        }
        for a in &p.areas {
            if r.contains_rect(&a.rect) && !areas.iter().any(|(id, _)| *id == a.id) {
                areas.push((a.id, a.rect));
            }
        }
    }
    Drag::Move { origin, devices, areas }
}

fn finish_move(st: &mut State, devices: &[(DeviceId, Pos)], areas: &[(AreaId, netvis_core::Rect)]) {
    let p = &st.doc.project;
    let mut edits = Vec::new();
    for (id, start) in devices {
        if let Some(d) = p.device(*id) {
            let mut before = d.clone();
            before.pos = *start;
            edits.push(Edit::Device { id: *id, before: Some(before), after: Some(d.clone()) });
        }
    }
    for (id, start) in areas {
        if let Some(a) = p.area(*id) {
            let mut before = a.clone();
            before.rect = *start;
            edits.push(Edit::Area { id: *id, before: Some(before), after: Some(a.clone()) });
        }
    }
    st.record_applied(Edit::Batch(edits), "Move");
}

fn finish_link(st: &mut State, from: Endpoint, w: Pos) {
    let p = &st.doc.project;
    let target = match hit_test(p, w, st.canvas.camera.zoom) {
        Hit::Port(ep) => Some(ep),
        Hit::Device(id) if id != from.device => {
            let media = p.endpoint_media(from);
            p.first_free_port(id, media).map(|port| Endpoint { device: id, port })
        }
        Hit::Device(_) => None,
        _ => return,
    };
    let Some(to) = target else {
        st.set_status("No free port on the target device");
        return;
    };
    match p.add_link_edit(from, to) {
        Ok((id, edit)) => {
            st.apply(edit, "Connect");
            st.sel.set_single(Item::Link(id));
        }
        Err(e) => st.set_status(format!("Cannot connect: {e}")),
    }
}

fn handle_clicks(response: &egui::Response, st: &mut State, hit: Hit, shift: bool) {
    if response.clicked() {
        st.doc.history.break_coalescing();
        match item_of(hit) {
            Some(item) => st.sel.click(item, shift),
            None if !shift => st.sel.clear(),
            None => {}
        }
    }
    if response.secondary_clicked()
        && let Some(item) = item_of(hit)
        && !st.sel.contains(item)
    {
        st.sel.set_single(item);
    }
}

fn handle_drop(ui: &Ui, response: &egui::Response, st: &mut State) {
    let Some(item) = response.dnd_release_payload::<PaletteItem>() else { return };
    let Some(ptr) = ui.input(|i| i.pointer.interact_pos()) else { return };
    let w = st.canvas.camera.to_world(response.rect.min, ptr);
    place_item(st, *item, w, true);
}

/// Add a palette item centred on `at` (world coordinates).
pub fn place_item(st: &mut State, item: PaletteItem, at: Pos, snap: bool) {
    let snap = snap && st.settings.snap_to_grid;
    let fix = |p: Pos| if snap { p.snapped(GRID) } else { p };
    match item {
        PaletteItem::Device(kind) => {
            let probe = Device::new(kind, "", Pos::default());
            let s = probe.size();
            let (id, edit) = st.doc.project.add_device_edit(kind, fix(at.offset(-s.x / 2.0, -s.y / 2.0)));
            st.apply(edit, &format!("Add {}", kind.label()));
            st.sel.set_single(Item::Device(id));
        }
        PaletteItem::Area(kind) => {
            let n = st.doc.project.areas.iter().filter(|a| a.kind == kind).count() + 1;
            let rect = netvis_core::Rect { min: fix(at.offset(-200.0, -130.0)), size: Pos::new(400.0, 260.0) };
            let area = Area::new(kind, format!("{} {n}", kind.label()), rect);
            let id = area.id;
            st.apply(Edit::Area { id, before: None, after: Some(area) }, &format!("Add {}", kind.label()));
            st.sel.set_single(Item::Area(id));
        }
    }
}

fn context_menu(response: &egui::Response, st: &mut State) {
    response.context_menu(|ui| {
        let has_devices = !st.sel.devices.is_empty();
        if ui.add_enabled(has_devices, egui::Button::new(format!("{} Duplicate", icons::COPY))).clicked() {
            st.duplicate_selection();
            ui.close();
        }
        if ui.add_enabled(has_devices, egui::Button::new(format!("{} Bring to front", icons::STACK))).clicked() {
            st.bring_to_front();
            ui.close();
        }
        if ui.add_enabled(!st.sel.is_empty(), egui::Button::new(format!("{} Delete", icons::TRASH))).clicked() {
            st.delete_selection();
            ui.close();
        }
        ui.separator();
        if ui.button(format!("{} Select all", icons::SELECTION)).clicked() {
            st.select_all();
            ui.close();
        }
        if ui.button(format!("{} Zoom to fit", icons::CORNERS_OUT)).clicked() {
            st.canvas.request_fit();
            ui.close();
        }
    });
}

fn port_tooltip(p: &Project, ep: Endpoint) -> Option<String> {
    let d = p.device(ep.device)?;
    let port = d.port(ep.port)?;
    let mut s = format!(
        "{}:{}  ·  {} {}",
        d.name,
        port.name,
        port.media.label(),
        netvis_core::port::format_speed(port.speed_mbps)
    );
    match &port.cfg.mode {
        SwitchportMode::Access { vlan } => s.push_str(&format!("\nAccess · VLAN {}", p.vlan_label(*vlan))),
        SwitchportMode::Trunk { native, .. } => s.push_str(&format!("\nTrunk · native {}", p.vlan_label(*native))),
        SwitchportMode::Routed => s.push_str("\nRouted"),
        SwitchportMode::Unset => {}
    }
    for a in &port.cfg.l3.addresses {
        s.push_str(&format!("\n{}", a.net));
    }
    if port.cfg.l3.dhcp_client {
        s.push_str("\nDHCP client");
    }
    match port.poe {
        PoeRole::Source(std) => s.push_str(&format!("\nPoE source · {}", std.label())),
        PoeRole::Powered(std) => s.push_str(&format!("\nPoE powered · {}", std.label())),
        PoeRole::None => {}
    }
    match p.port_link(ep.port) {
        Some(l) => {
            let far = if l.a.port == ep.port { l.b } else { l.a };
            if let Some(fd) = p.device(far.device) {
                let fp = fd.port(far.port).map(|x| x.name.as_str()).unwrap_or("?");
                s.push_str(&format!("\n→ {}:{}  ({})", fd.name, fp, l.cable.label()));
            }
        }
        None => s.push_str("\nFree — drag to another port to connect"),
    }
    Some(s)
}

// ---- painting ------------------------------------------------------------

fn draw_grid(painter: &egui::Painter, rect: Rect, cam: &Camera, pal: &Palette) {
    let mut step = GRID;
    while step * cam.zoom < 10.0 {
        step *= 5.0;
    }
    let major = step * 5.0;
    let tl = cam.to_world(rect.min, rect.min);
    let br = cam.to_world(rect.min, rect.max);

    let mut x = (tl.x / step).floor() * step;
    while x <= br.x {
        let sx = cam.to_screen(rect.min, Pos::new(x, 0.0)).x;
        let is_major = (x / major).round() * major == x;
        let c = if is_major { pal.grid_major } else { pal.grid_minor };
        painter.line_segment([pos2(sx, rect.top()), pos2(sx, rect.bottom())], Stroke::new(1.0, c));
        x += step;
    }
    let mut y = (tl.y / step).floor() * step;
    while y <= br.y {
        let sy = cam.to_screen(rect.min, Pos::new(0.0, y)).y;
        let is_major = (y / major).round() * major == y;
        let c = if is_major { pal.grid_major } else { pal.grid_minor };
        painter.line_segment([pos2(rect.left(), sy), pos2(rect.right(), sy)], Stroke::new(1.0, c));
        y += step;
    }
}

fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

fn draw_area(painter: &egui::Painter, cam: &Camera, origin: Pos2, a: &Area, selected: bool, pal: &Palette) {
    let z = cam.zoom;
    let r = cam.rect_to_screen(origin, a.rect);
    let color = rgb(a.color);
    let radius = CornerRadius::same((8.0 * z).clamp(2.0, 12.0) as u8);
    painter.rect_filled(r, radius, color.gamma_multiply(0.07));
    let stroke = if selected { Stroke::new(2.0, pal.accent) } else { Stroke::new(1.2, color.gamma_multiply(0.7)) };
    painter.rect_stroke(r, radius, stroke, StrokeKind::Inside);

    let header = Rect::from_min_size(r.min, vec2(r.width(), AREA_HEADER * z));
    let hr = CornerRadius { nw: radius.nw, ne: radius.ne, sw: 0, se: 0 };
    painter.rect_filled(header, hr, color.gamma_multiply(0.16));
    if 12.0 * z >= 5.0 {
        painter.text(
            header.left_center() + vec2(8.0 * z, 0.0),
            Align2::LEFT_CENTER,
            format!("{}  {}", icons::area(a.kind), a.name),
            FontId::proportional(12.5 * z),
            pal.text,
        );
        painter.text(
            header.right_center() - vec2(8.0 * z, 0.0),
            Align2::RIGHT_CENTER,
            a.kind.label(),
            FontId::proportional(10.5 * z),
            pal.subtext,
        );
    }
    // Resize grip.
    let br = r.max;
    let s = AREA_HANDLE * z * 0.7;
    let grip = Stroke::new(1.2, color.gamma_multiply(0.8));
    painter.line_segment([br - vec2(s, 3.0), br - vec2(3.0, s)], grip);
    painter.line_segment([br - vec2(s * 0.55, 3.0), br - vec2(3.0, s * 0.55)], grip);
}

#[allow(clippy::too_many_arguments)]
fn draw_link(
    painter: &egui::Painter,
    cam: &Camera,
    origin: Pos2,
    p: &Project,
    l: &Link,
    selected: bool,
    hovered: bool,
    pal: &Palette,
) {
    let Some((a, b)) = link_points(p, l) else { return };
    let (sa, sb) = (cam.to_screen(origin, a), cam.to_screen(origin, b));
    let family = l.cable.family();
    let color = theme::cable_color(family);
    let width = (2.0 * cam.zoom).clamp(1.2, 4.0);

    if selected || hovered {
        let halo = if selected { pal.accent.gamma_multiply(0.45) } else { color.gamma_multiply(0.3) };
        painter.line_segment([sa, sb], Stroke::new(width + 5.0, halo));
    }
    let stroke = Stroke::new(width, color);
    use netvis_core::link::CableFamily as F;
    match family {
        F::Wireless | F::Logical => painter.extend(Shape::dashed_line(&[sa, sb], stroke, 7.0 * cam.zoom.max(0.5), 5.0 * cam.zoom.max(0.5))),
        F::Console => painter.extend(Shape::dotted_line(&[sa, sb], color, 4.0 * cam.zoom.max(0.5), width / 2.0)),
        _ => {
            painter.line_segment([sa, sb], stroke);
        }
    }

    let mut text = l.label.clone();
    if let Some(m) = l.length_m {
        if !text.is_empty() {
            text.push_str(" · ");
        }
        text.push_str(&format!("{m} m"));
    }
    if !text.is_empty() && 10.0 * cam.zoom >= 6.0 {
        let mid = sa + (sb - sa) / 2.0;
        let galley = painter.layout_no_wrap(text, FontId::proportional(10.5 * cam.zoom), pal.text);
        let r = Align2::CENTER_CENTER.anchor_size(mid, galley.size()).expand(3.0);
        painter.rect_filled(r, 3.0, pal.canvas_bg.gamma_multiply(0.92));
        painter.rect_stroke(r, 3.0, Stroke::new(1.0, color.gamma_multiply(0.6)), StrokeKind::Inside);
        painter.galley(r.min + vec2(3.0, 3.0), galley, pal.text);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_device(
    painter: &egui::Painter,
    cam: &Camera,
    origin: Pos2,
    p: &Project,
    d: &Device,
    selected: bool,
    hover_port: Option<Endpoint>,
    show_addresses: bool,
    pal: &Palette,
) {
    let z = cam.zoom;
    let r = cam.rect_to_screen(origin, d.rect());
    if !painter.clip_rect().intersects(r) {
        return;
    }
    let cat = theme::category_color(d.kind.category());
    let radius = CornerRadius::same((7.0 * z).clamp(2.0, 10.0) as u8);

    // Shadow, body, category stripe.
    painter.rect_filled(r.translate(vec2(0.0, 2.0 * z)), radius, Color32::from_black_alpha(18));
    painter.rect_filled(r, radius, pal.card);
    let stripe = Rect::from_min_size(r.min, vec2(4.0 * z, r.height()));
    painter.rect_filled(stripe, CornerRadius { nw: radius.nw, sw: radius.sw, ne: 0, se: 0 }, cat);
    let border = if selected { Stroke::new(2.0, pal.accent) } else { Stroke::new(1.0, pal.card_border) };
    painter.rect_stroke(r, radius, border, StrokeKind::Inside);

    // Header: icon badge, name, kind.
    if 13.0 * z >= 4.0 {
        let badge = Rect::from_min_size(r.min + vec2(12.0, 9.0) * z, Vec2::splat(30.0 * z));
        painter.rect_filled(badge, CornerRadius::same((6.0 * z) as u8), cat.gamma_multiply(0.14));
        painter.text(badge.center(), Align2::CENTER_CENTER, icons::device(d.kind), FontId::proportional(18.0 * z), cat);

        let tx = badge.right() + 8.0 * z;
        let max_w = r.right() - tx - 6.0 * z;
        let name = painter.layout(
            d.name.clone(),
            FontId::proportional(13.0 * z),
            pal.text,
            f32::INFINITY,
        );
        let name_pos = pos2(tx, badge.top() + 1.0 * z);
        if name.size().x <= max_w {
            painter.galley(name_pos, name, pal.text);
        } else {
            let clipped = painter.with_clip_rect(Rect::from_min_size(name_pos, vec2(max_w, 20.0 * z)));
            clipped.galley(name_pos, name, pal.text);
        }
        let addr = show_addresses.then(|| d.primary_address()).flatten();
        let sub = match addr {
            Some(a) => a.net.to_string(),
            None if d.model.is_empty() => d.kind.label().to_owned(),
            None => d.model.clone(),
        };
        let sub_painter = painter.with_clip_rect(Rect::from_min_size(pos2(tx, badge.top()), vec2(max_w, 40.0 * z)));
        sub_painter.text(
            pos2(tx, badge.bottom() - 1.0 * z),
            Align2::LEFT_BOTTOM,
            sub,
            FontId::proportional(10.5 * z),
            pal.subtext,
        );
    }

    // Port markers.
    if z >= PORT_DETAIL_ZOOM {
        let size = layout::PORT_SIZE * z;
        for (i, port) in d.ports.iter().enumerate() {
            let c = cam.to_screen(origin, d.port_center(i));
            let link = p.port_link(port.id);
            let fill = link.map_or(pal.port_free, |l| theme::cable_color(l.cable.family()));
            let hovered = hover_port.is_some_and(|h| h.port == port.id);
            let vlan_color = match &port.cfg.mode {
                SwitchportMode::Access { vlan: Some(v) } => p.vlan(*v).map(|v| rgb(v.color)),
                SwitchportMode::Trunk { .. } => Some(pal.text),
                _ => None,
            };
            let stroke = if hovered {
                Stroke::new(2.0, pal.accent)
            } else if let Some(c) = vlan_color {
                Stroke::new(1.6, c)
            } else if matches!(port.poe, PoeRole::Source(_)) {
                Stroke::new(1.0, pal.poe)
            } else {
                Stroke::new(1.0, pal.port_stroke)
            };
            if theme::media_is_round(port.media) {
                painter.circle_filled(c, size / 2.0, fill);
                painter.circle_stroke(c, size / 2.0, stroke);
            } else {
                let pr = Rect::from_center_size(c, Vec2::splat(size));
                let cr = if port.media.is_optical_cage() { 0.0 } else { 1.5 * z };
                painter.rect_filled(pr, cr, fill);
                painter.rect_stroke(pr, cr, stroke, StrokeKind::Inside);
            }
        }
    }
}

/// Red/amber dots on the devices and links that are the subject of an error
/// or warning (info-level findings stay in the Issues panel).
fn draw_issue_badges(painter: &egui::Painter, cam: &Camera, origin: Pos2, p: &Project, issues: &[netvis_validate::Issue]) {
    use netvis_validate::Severity;
    let mut worst: std::collections::HashMap<Item, Severity> = std::collections::HashMap::new();
    for i in issues.iter().filter(|i| i.severity != Severity::Info) {
        if let Some(item) = i.items.first() {
            let e = worst.entry(*item).or_insert(i.severity);
            *e = (*e).min(i.severity);
        }
    }
    let r = (6.5 * cam.zoom).clamp(4.5, 9.0);
    let badge = |at: Pos2, sev: Severity| {
        let c = if sev == Severity::Error { crate::widgets::ERROR } else { crate::widgets::WARN };
        painter.circle_filled(at, r + 1.5, Color32::WHITE);
        painter.circle_filled(at, r, c);
        painter.text(at, Align2::CENTER_CENTER, "!", FontId::proportional(r * 1.5), Color32::WHITE);
    };
    for (item, sev) in worst {
        match item {
            Item::Device(id) => {
                if let Some(d) = p.device(id) {
                    let rect = cam.rect_to_screen(origin, d.rect());
                    badge(rect.right_top() + vec2(-2.0, 2.0), sev);
                }
            }
            Item::Link(id) => {
                if let Some((a, b)) = p.link(id).and_then(|l| link_points(p, l)) {
                    let (sa, sb) = (cam.to_screen(origin, a), cam.to_screen(origin, b));
                    badge(sa + (sb - sa) * 0.35, sev);
                }
            }
            Item::Area(id) => {
                if let Some(a) = p.area(id) {
                    let rect = cam.rect_to_screen(origin, a.rect);
                    badge(rect.right_top() + vec2(-10.0, 10.0), sev);
                }
            }
            _ => {}
        }
    }
}

// ---- minimap -------------------------------------------------------------

fn minimap(ui: &mut Ui, canvas: Rect, st: &mut State, pal: &Palette) {
    let p = &st.doc.project;
    let Some(content) = p.bounds() else { return };
    let cam = st.canvas.camera;
    let tl = cam.to_world(canvas.min, canvas.min);
    let br = cam.to_world(canvas.min, canvas.max);
    let view = netvis_core::Rect::new(tl.x, tl.y, br.x - tl.x, br.y - tl.y);
    let world = content.union(&view);

    let size = vec2(200.0, 130.0);
    let mm = Rect::from_min_size(canvas.max - size - vec2(12.0, 12.0), size);
    if mm.left() < canvas.left() || mm.top() < canvas.top() {
        return;
    }
    let scale = ((size.x - 12.0) / world.size.x).min((size.y - 12.0) / world.size.y);
    let off = mm.center() - vec2(world.size.x, world.size.y) * scale / 2.0;
    let map = |w: Pos| off + vec2(w.x - world.min.x, w.y - world.min.y) * scale;
    let map_rect = |r: netvis_core::Rect| Rect::from_min_max(map(r.min), map(r.max()));

    let painter = ui.painter_at(mm);
    painter.rect_filled(mm, 8.0, pal.card.gamma_multiply(0.94));
    painter.rect_stroke(mm, 8.0, Stroke::new(1.0, pal.card_border), StrokeKind::Inside);
    for a in &p.areas {
        painter.rect_filled(map_rect(a.rect), 1.0, rgb(a.color).gamma_multiply(0.25));
    }
    for d in &p.devices {
        painter.rect_filled(map_rect(d.rect()), 1.0, theme::category_color(d.kind.category()));
    }
    painter.rect_stroke(map_rect(view), 1.0, Stroke::new(1.5, pal.accent), StrokeKind::Middle);

    let resp = ui.interact(mm, ui.id().with("minimap"), Sense::click_and_drag());
    if (resp.clicked() || resp.dragged())
        && let Some(ptr) = resp.interact_pointer_pos()
    {
        let w = Pos::new((ptr.x - off.x) / scale + world.min.x, (ptr.y - off.y) / scale + world.min.y);
        st.canvas.camera.center_on(canvas, w);
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::Move);
    }
}
