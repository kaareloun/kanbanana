//! Live preview markdown editor. It edits markdown text but never shows the syntax:
//! the layout hides the marks and `overlay` paints bullets, checkboxes and images in
//! their place. egui has no rich text widget, so this sits on top of a TextEdit.

pub mod caret;
pub mod commands;
pub mod layout;
pub mod overlay;
pub mod parse;
pub mod table;
pub mod toolbar;

use std::collections::HashMap;
use std::ops::Range;

use eframe::egui::{
    self, Event, Id, Key, Modifiers, Pos2, Response, Shape, TextBuffer, TextEdit, Ui,
    text::{CCursor, CCursorRange},
    text_edit::TextEditState,
    vec2,
};

use commands::{Edit, ListKind, byte_to_char, char_to_byte};
use overlay::Pic;
use toolbar::{LinkEvent, Tool};

use crate::theme::{Theme, W, font};

/// Text sizes of one editor surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub size: f32,
    pub line: f32,
    /// False leaves out the heading tools and shortcuts, for comments. Headings in the
    /// text show as headings either way.
    pub headings: bool,
    /// Font size and line height of heading levels 1, 2 and 3 and below.
    pub h: [(f32, f32); 3],
}

pub const DESCRIPTION: Look =
    Look { size: 14.0, line: 23.0, headings: true, h: [(24.0, 34.0), (19.0, 29.0), (16.0, 26.0)] };
pub const COMMENT: Look =
    Look { size: 13.0, line: 21.0, headings: false, h: [(18.0, 28.0), (16.0, 25.0), (14.0, 23.0)] };

pub struct Options<'a> {
    pub look: Look,
    /// Shown in muted text while the text is empty.
    pub placeholder: &'a str,
    /// Minimum height in lines. 1 makes a single line field that grows with its text.
    pub min_rows: usize,
    /// Uploads that still run, as (file name, progress 0 to 1).
    pub uploads: &'a [(String, Option<f32>)],
    pub request_focus: bool,
    /// Byte range to select, for example to restore a caret.
    pub select: Option<Range<usize>>,
}

impl Default for Options<'_> {
    fn default() -> Self {
        Options { look: DESCRIPTION, placeholder: "", min_rows: 1, uploads: &[], request_focus: false, select: None }
    }
}

pub struct RichResponse {
    /// The TextEdit response. Pass it to `paste::handle` for image paste and drop.
    pub response: Response,
    /// The text changed this frame, by typing or by a command.
    pub changed: bool,
    pub focused: bool,
    #[allow(dead_code)]
    pub gained_focus: bool,
    pub lost_focus: bool,
    /// A link the user clicked.
    pub open_url: Option<String>,
}

/// Editor state between frames.
#[derive(Clone, Default)]
struct Mem {
    /// Caret byte offset of the last frame, for the caret rules.
    caret: usize,
    link: Option<LinkPrompt>,
    /// A selection from `Options::select` that waits for focus.
    select: Option<Range<usize>>,
}

#[derive(Clone)]
struct LinkPrompt {
    sel: Range<usize>,
    url: String,
    anchor: Pos2,
    first: bool,
}

pub fn editor(ui: &mut Ui, t: &Theme, id: Id, text: &mut String, opts: &Options) -> RichResponse {
    let ctx = ui.ctx().clone();
    let mem_id = id.with("rich");
    let mut mem: Mem = ctx.data(|d| d.get_temp(mem_id)).unwrap_or_default();
    let look = opts.look;
    let was_focused = ctx.memory(|m| m.has_focus(id));
    let mut changed = false;

    if let Some(sel) = &opts.select {
        mem.select = Some(sel.clone());
    }
    // An unfocused TextEdit collapses its selection to the caret, so a requested
    // selection waits until the editor has focus.
    if was_focused && let Some(sel) = mem.select.take().filter(|s| s.end <= text.len()) {
        set_sel(&ctx, id, text, sel.clone());
        mem.caret = sel.end;
    }
    if was_focused && mem.link.is_none() {
        let sel = selection(&ctx, id, text);
        if let Some(cmd) = shortcut(ui, look.headings) {
            run_tool(&ctx, id, text, cmd, sel.clone(), &mut mem, None);
            changed |= cmd != Tool::Link;
        } else if let Some(e) = edit_key(ui, text, look.headings, sel) {
            apply(&ctx, id, text, e);
            changed = true;
        }
    }

    let width = ui.available_width();
    let mut pics: HashMap<String, Pic> = HashMap::new();
    for l in parse::parse(text, true) {
        if let Some(img) = l.image {
            pics.entry(img.url.clone()).or_insert_with(|| overlay::pic(&ctx, &img));
        }
    }

    let bg = ui.painter().add(Shape::Noop);
    let theme = *t;
    let mut layouter = |ui: &Ui, buf: &dyn TextBuffer, wrap: f32| {
        let s = buf.as_str();
        let lines = parse::parse(s, true);
        let measure = |s: &str, f: &egui::FontId| measure(ui, s, f);
        let job = layout::job(s, &lines, &theme, &look, wrap, &|img| overlay::image_line_h(&pics, img, wrap), &measure);
        ui.fonts_mut(|f| f.layout_job(job))
    };
    let hint = egui::RichText::new(opts.placeholder).font(font(W::Regular, look.size)).color(t.muted);
    let out = TextEdit::multiline(text)
        .id(id)
        .frame(egui::Frame::NONE)
        .margin(0)
        .desired_rows(opts.min_rows)
        .desired_width(width)
        .lock_focus(true)
        .font(font(W::Regular, look.size))
        .hint_text(hint)
        .layouter(&mut layouter)
        .show(ui);
    let response = out.response.response.clone();
    // After the TextEdit, so the key that asked for focus (E) is not typed into it.
    if opts.request_focus {
        ctx.memory_mut(|m| m.request_focus(id));
    }
    changed |= response.changed();
    // Memory focus, not `Response::has_focus`, so the editor stays focused while the
    // window is in the background.
    let focused = ctx.memory(|m| m.has_focus(id));

    let galley = out.galley.clone();
    let shown = galley.text().to_string();
    let lines = parse::parse(&shown, true);
    let origin = out.galley_pos - vec2(galley.rect.left(), 0.0);
    let links_live = !focused || ui.input(|i| i.modifiers.command);
    let paint_ctx = overlay::Ctx {
        t,
        look: &look,
        id,
        text: &shown,
        base: 0,
        lines: &lines,
        galley: &galley,
        origin,
        width: galley.job.wrap.max_width.min(width),
        pics: &pics,
        uploads: opts.uploads,
        links_live,
    };
    let clicks = overlay::paint(ui, &paint_ctx, bg);
    let row = |b: usize| {
        let c = byte_to_char(&shown, b);
        galley.pos_from_cursor(CCursor::new(c)).translate(origin.to_vec2())
    };

    if let Some(li) = clicks.toggle
        && *text == shown
        && let Some(s) = commands::toggle_check(text, &lines[li])
    {
        *text = s;
        changed = true;
        if was_focused {
            ctx.memory_mut(|m| m.request_focus(id));
        }
    }

    // Caret rules and the edits that follow typing.
    let mut sel = out.cursor_range.filter(|_| focused && *text == shown).map(|r| {
        (char_to_byte(text, r.primary.index.0), char_to_byte(text, r.secondary.index.0))
    });
    if let Some((primary, secondary)) = sel {
        let mut at = caret::snap(&lines, mem.caret, primary);
        if caret::on_hidden_line(&lines, at) && at == text.len() && at > mem.caret {
            text.push('\n');
            at = text.len();
            changed = true;
        }
        let mut edit = None;
        if response.changed() && primary == secondary {
            edit = commands::auto_block(text, at);
        }
        if edit.is_none() {
            let now = parse::parse(text, true);
            edit = commands::drop_empty_marks(text, &now, at);
        }
        if let Some(e) = edit {
            at = e.sel.end;
            *text = e.text;
            changed = true;
            set_sel(&ctx, id, text, at..at);
        } else if at != primary {
            let anchor = if primary == secondary { at } else { secondary };
            set_sel(&ctx, id, text, anchor..at);
        }
        mem.caret = at;
        sel = Some((if primary == secondary { at } else { secondary }, at));
    }
    if response.lost_focus() {
        let now = parse::parse(text, true);
        if let Some(e) = commands::drop_empty_marks(text, &now, usize::MAX) {
            *text = e.text;
            changed = true;
        }
    }

    // The bar stays for the frame of a click on it, when the TextEdit already let go.
    let selected = sel.or_else(|| {
        let r = out.cursor_range?;
        was_focused.then(|| (char_to_byte(text, r.secondary.index.0), char_to_byte(text, r.primary.index.0)))
    });
    let dragging = ui.input(|i| i.pointer.primary_down());
    if let Some((a, b)) = selected
        && a != b
        && mem.link.is_none()
        && !dragging
        && *text == shown
    {
        let (lo, hi) = (a.min(b), a.max(b));
        let (ra, rb) = (row(lo), row(hi));
        let x = if (ra.min.y - rb.min.y).abs() < 1.0 { (ra.min.x + rb.min.x) / 2.0 } else { ra.min.x };
        if let Some(tool) = toolbar::show(&ctx, t, id, Pos2::new(x, ra.min.y), look.headings) {
            let anchor = Pos2::new(ra.min.x, rb.max.y);
            run_tool(&ctx, id, text, tool, lo..hi, &mut mem, Some(anchor));
            changed |= tool != Tool::Link;
            ctx.memory_mut(|m| m.request_focus(id));
        }
    }
    if let Some(prompt) = &mut mem.link {
        if prompt.anchor == Pos2::ZERO {
            let r = row(prompt.sel.end.min(shown.len()));
            prompt.anchor = Pos2::new(row(prompt.sel.start.min(shown.len())).min.x, r.max.y);
        }
        let first = std::mem::replace(&mut prompt.first, false);
        let ev = toolbar::link_prompt(&ctx, t, id, prompt.anchor, &mut prompt.url, first);
        if let Some(ev) = ev
            && let Some(p) = mem.link.take()
        {
            if let LinkEvent::Apply(url) = ev
                && p.sel.end <= text.len()
            {
                let e = commands::link(text, p.sel, &url);
                mem.caret = e.sel.end;
                apply(&ctx, id, text, e);
                changed = true;
            }
            ctx.memory_mut(|m| m.request_focus(id));
        }
    }

    ctx.data_mut(|d| d.insert_temp(mem_id, mem));
    if changed {
        ctx.request_repaint();
    }
    RichResponse {
        gained_focus: response.gained_focus(),
        lost_focus: response.lost_focus(),
        response,
        changed,
        focused,
        open_url: clicks.open_url,
    }
}

/// Width of `s` in font `f`, for the table columns.
pub fn measure(ui: &Ui, s: &str, f: &egui::FontId) -> f32 {
    ui.fonts_mut(|fo| fo.layout_no_wrap(s.to_string(), f.clone(), egui::Color32::WHITE).size().x)
}

/// Runs a toolbar tool or shortcut. Link opens the url popover.
fn run_tool(ctx: &egui::Context, id: Id, text: &mut String, tool: Tool, sel: Range<usize>, mem: &mut Mem, anchor: Option<Pos2>) {
    let e = match tool {
        Tool::Bold => commands::wrap(text, sel, "**"),
        Tool::Code => commands::code(text, sel),
        Tool::Heading => commands::heading(text, sel.start, 2),
        Tool::Bullet => commands::prefix_lines(text, sel, ListKind::Bullet),
        Tool::Check => commands::prefix_lines(text, sel, ListKind::Check),
        Tool::Link => {
            let url = String::new();
            mem.link = Some(LinkPrompt { sel, url, anchor: anchor.unwrap_or(Pos2::ZERO), first: true });
            return;
        }
    };
    mem.caret = e.sel.end;
    apply(ctx, id, text, e);
}

/// Formatting shortcuts. Each one maps to a toolbar tool.
fn shortcut(ui: &Ui, headings: bool) -> Option<Tool> {
    let cmd = Modifiers::COMMAND;
    if take_key(ui, Key::B, cmd) {
        return Some(Tool::Bold);
    }
    if take_key(ui, Key::E, cmd) {
        return Some(Tool::Code);
    }
    if take_key(ui, Key::K, cmd) {
        return Some(Tool::Link);
    }
    if take_key(ui, Key::Num8, cmd | Modifiers::SHIFT) {
        return Some(Tool::Bullet);
    }
    if take_key(ui, Key::Num9, cmd | Modifiers::SHIFT) {
        return Some(Tool::Check);
    }
    if headings && take_key(ui, Key::Num2, cmd | Modifiers::ALT) {
        return Some(Tool::Heading);
    }
    None
}

/// Keys that edit the text in a markdown aware way. They run before the TextEdit, so it
/// does not see the keys it should not handle.
fn edit_key(ui: &Ui, text: &str, headings: bool, sel: Range<usize>) -> Option<Edit> {
    let none = Modifiers::NONE;
    let cmd = Modifiers::COMMAND;
    if take_key(ui, Key::Num7, cmd | Modifiers::SHIFT) {
        return Some(commands::prefix_lines(text, sel, ListKind::Numbered));
    }
    if headings {
        for (k, level) in [(Key::Num1, 1), (Key::Num3, 3)] {
            if take_key(ui, k, cmd | Modifiers::ALT) {
                return Some(commands::heading(text, sel.end, level));
            }
        }
    }
    if !sel.is_empty() {
        return None;
    }
    let lines = parse::parse(text, true);
    let at = sel.end;
    // Each key is taken only when the command applies, else the TextEdit handles it.
    let ev = |k: Key| pressed(ui, k, none);
    if ev(Key::Enter) {
        let e = commands::enter(text, &lines, at)?;
        take_key(ui, Key::Enter, none);
        return Some(e);
    }
    if ev(Key::Backspace) {
        let e = commands::backspace(text, &lines, at)?;
        take_key(ui, Key::Backspace, none);
        return Some(e);
    }
    if ev(Key::Delete) {
        let e = commands::delete(text, &lines, at)?;
        take_key(ui, Key::Delete, none);
        return Some(e);
    }
    None
}

/// Matches the modifiers exactly. Shift+7 types another symbol on many layouts, so the
/// physical key counts too.
fn key_matches(e: &Event, k: Key, m: Modifiers) -> bool {
    matches!(e, Event::Key { key, physical_key, pressed: true, modifiers, .. }
        if (*key == k || *physical_key == Some(k))
            && modifiers.command == m.command
            && modifiers.shift == m.shift
            && modifiers.alt == m.alt)
}

fn pressed(ui: &Ui, k: Key, m: Modifiers) -> bool {
    ui.input(|i| i.events.iter().any(|e| key_matches(e, k, m)))
}

fn take_key(ui: &Ui, k: Key, m: Modifiers) -> bool {
    ui.input_mut(|i| {
        let before = i.events.len();
        i.events.retain(|e| !key_matches(e, k, m));
        i.events.len() != before
    })
}

/// The selection as a byte range, anchor first. The text end when there is no cursor yet.
fn selection(ctx: &egui::Context, id: Id, text: &str) -> Range<usize> {
    let range = TextEditState::load(ctx, id).and_then(|s| s.cursor.char_range());
    let Some(r) = range else { return text.len()..text.len() };
    let (a, b) = (r.primary.index.0.min(r.secondary.index.0), r.primary.index.0.max(r.secondary.index.0));
    char_to_byte(text, a)..char_to_byte(text, b)
}

/// Selects `sel` (bytes, anchor to caret) in the TextEdit.
fn set_sel(ctx: &egui::Context, id: Id, text: &str, sel: Range<usize>) {
    let (a, b) = (byte_to_char(text, sel.start), byte_to_char(text, sel.end));
    let mut st = TextEditState::load(ctx, id).unwrap_or_default();
    st.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(a), CCursor::new(b))));
    st.store(ctx, id);
}

fn apply(ctx: &egui::Context, id: Id, text: &mut String, e: Edit) {
    *text = e.text;
    set_sel(ctx, id, text, e.sel);
}
