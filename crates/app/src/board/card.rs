//! Issue card: status and a two-line title on top; created date, priority, labels, dev stage
//! icon and assignee in the bottom row. Every card has the same height. A hover tooltip has the rest.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Datelike, Local, Utc};
use eframe::egui::{
    Align2, Color32, CornerRadius, Galley, Painter, Rect, RichText, Sense, Shadow, Stroke, StrokeKind, Ui, vec2,
    text::{LayoutJob, TextFormat, TextWrapping},
    pos2,
};
use model::{BoardData, DevStage, DevState, Issue};

use super::layout::status_kind;
use crate::theme::parts::{self, Icon, StatusKind};
use crate::theme::{Theme, W, font};

const PAD_X: f32 = 14.0;
const PAD_Y: f32 = 12.0;
const LINE_H: f32 = 20.0;
const ROW_H: f32 = 20.0;
const TAG_GAP: f32 = 6.0;
const ITEM_GAP: f32 = 8.0;
const AVATAR: f32 = 20.0;
/// Space for the status icon in front of the title.
const INDENT: f32 = 22.0;
const LIFT: f32 = 1.5;
pub const RADIUS: u8 = 10;
/// Fixed card height. The title sits at the top, the bottom row at the bottom.
pub const HEIGHT: f32 = 103.0;

pub struct Who<'a> {
    pub initials: String,
    pub color: Color32,
    pub avatar_url: Option<&'a str>,
}

/// Everything one card shows, resolved from ids.
pub struct CardData<'a> {
    pub issue: &'a Issue,
    pub status: StatusKind,
    pub assignee: Option<Who<'a>>,
    pub labels: Vec<(&'a str, Color32)>,
    pub dev: Option<&'a DevState>,
}

/// Animation state of one card. Each value runs from 0 to 1.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Fx {
    pub hover: f32,
    pub select: f32,
    /// Keyboard focus ring around the card.
    pub focus: f32,
    /// The card follows the pointer. It paints its own shadow.
    pub floating: bool,
}

impl Fx {
    pub const FLOATING: Fx = Fx { hover: 0.0, select: 1.0, focus: 0.0, floating: true };
}

/// Lookups the cards need. Built once per frame.
pub struct Resolver<'a> {
    users: HashMap<&'a str, (String, Color32, Option<&'a str>)>,
    labels: HashMap<&'a str, (&'a str, Color32)>,
    states: HashMap<&'a str, StatusKind>,
    dev: HashMap<&'a str, &'a DevState>,
}

impl<'a> Resolver<'a> {
    pub fn new(data: &'a BoardData, t: &Theme) -> Self {
        let palette = [t.iris, t.foam, t.gold, t.rose, t.pine, t.love];
        let users = data
            .users
            .iter()
            .enumerate()
            .map(|(i, u)| (u.id.as_str(), (initials(&u.name), palette[i % palette.len()], u.avatar_url.as_deref())))
            .collect();
        let labels = data
            .labels
            .iter()
            .map(|l| (l.id.as_str(), (l.name.as_str(), t.nearest(&l.color))))
            .collect();
        let states = data.states.iter().map(|s| (s.id.as_str(), status_kind(s.kind, &s.name))).collect();
        let dev = data.dev.iter().map(|d| (d.identifier.as_str(), d)).collect();
        Resolver { users, labels, states, dev }
    }

    pub fn card(&self, issue: &'a Issue) -> CardData<'a> {
        let assignee = issue.assignee_id.as_deref().and_then(|id| self.users.get(id)).map(|(s, c, url)| Who {
            initials: s.clone(),
            color: *c,
            avatar_url: *url,
        });
        CardData {
            issue,
            status: self.states.get(issue.state_id.as_str()).copied().unwrap_or(StatusKind::Todo),
            assignee,
            labels: issue.label_ids.iter().filter_map(|id| self.labels.get(id.as_str()).copied()).collect(),
            dev: self.dev.get(issue.identifier.as_str()).copied(),
        }
    }
}

pub fn initials(name: &str) -> String {
    let words: Vec<&str> = name.split_whitespace().collect();
    let s: String = match words.as_slice() {
        [] => "?".into(),
        [one] => one.chars().take(2).collect(),
        [a, b, ..] => a.chars().take(1).chain(b.chars().take(1)).collect(),
    };
    s.to_uppercase()
}

/// "Today", "Yesterday", "12 Nov", or "12 Nov 2025" before this year. Local time.
pub fn created_label(at: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let (day, today) = (at.with_timezone(&Local), now.with_timezone(&Local).date_naive());
    if day.date_naive() == today {
        "Today".into()
    } else if today.pred_opt() == Some(day.date_naive()) {
        "Yesterday".into()
    } else if day.year() == today.year() {
        day.format("%-d %b").to_string()
    } else {
        day.format("%-d %b %Y").to_string()
    }
}

/// Hover tooltip: identifier, title, comments, dev stage and both dates.
pub fn tooltip(ui: &mut Ui, t: &Theme, c: &CardData) {
    let full = |d: DateTime<Utc>| d.with_timezone(&Local).format("%-d %b %Y, %H:%M").to_string();
    let small = |s: String, c: Color32| RichText::new(s).font(font(W::Medium, 12.0)).color(c);
    ui.set_max_width(320.0);
    ui.spacing_mut().item_spacing.y = 4.0;
    ui.label(RichText::new(&c.issue.identifier).font(font(W::MonoMedium, 12.0)).color(t.subtle));
    ui.label(RichText::new(&c.issue.title).font(font(W::SemiBold, 13.0)).color(t.text));
    let n = c.issue.comment_count;
    let comments = if n == 1 { "1 comment".to_string() } else { format!("{n} comments") };
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
        parts::icon(ui.painter(), r.center(), Icon::Comment, 14.0, t.subtle);
        ui.label(small(comments, t.subtle));
    });
    if let Some(d) = c.dev {
        let text = match d.git_ref.as_deref().filter(|r| !r.is_empty()) {
            Some(r) => format!("{}  {r}", d.stage.label()),
            None => d.stage.label().to_string(),
        };
        ui.label(RichText::new(text).font(font(W::MonoMedium, 12.0)).color(t.stage(d.stage)));
    }
    ui.label(small(format!("Created {}", full(c.issue.created_at)), t.muted));
    ui.label(small(format!("Updated {}", full(c.issue.updated_at)), t.muted));
}

fn title_galley(p: &Painter, t: &Theme, title: &str, width: f32, bold: bool) -> Arc<Galley> {
    let w = if bold { W::SemiBold } else { W::Medium };
    let mut job = LayoutJob::single_section(
        title.to_string(),
        TextFormat { font_id: font(w, 14.0), color: t.text, line_height: Some(LINE_H), ..Default::default() },
    );
    job.wrap = TextWrapping { max_width: width, max_rows: 2, break_anywhere: false, overflow_character: Some('…') };
    p.layout_job(job)
}

/// Cuts `s` and adds "…" until `fits` accepts it. Returns "" when nothing fits.
fn ellipsize(s: &str, fits: impl Fn(&str) -> bool) -> String {
    if fits(s) {
        return s.to_string();
    }
    let mut chars: Vec<char> = s.chars().collect();
    while !chars.is_empty() {
        chars.pop();
        let cut = format!("{}…", chars.iter().collect::<String>().trim_end());
        if fits(&cut) {
            return cut;
        }
    }
    String::new()
}

/// The labels that fit in one row, then "+N" for the rest. A single long label ends in "…".
fn label_row(p: &Painter, labels: &[(&str, Color32)], width: f32) -> (Vec<(String, Color32)>, usize) {
    let n = labels.len();
    let w = |s: &str| parts::tag_size(p, s, false, false).x;
    let more = |k: usize| if k < n { TAG_GAP + w(&format!("+{}", n - k)) } else { 0.0 };
    for k in (1..=n).rev() {
        let used: f32 = labels[..k].iter().map(|(s, _)| w(s)).sum::<f32>() + (k - 1) as f32 * TAG_GAP;
        if used + more(k) <= width {
            let shown = labels[..k].iter().map(|(s, c)| (s.to_string(), *c)).collect();
            return (shown, n - k);
        }
    }
    let Some((first, c)) = labels.first() else { return (Vec::new(), 0) };
    let cut = ellipsize(first, |s| w(s) + more(1) <= width);
    if cut.is_empty() { (Vec::new(), n) } else { (vec![(cut, *c)], n - 1) }
}

fn card_shadows(t: &Theme, hover: f32) -> [Shadow; 3] {
    let a = |x: f32| (x * 255.0) as u8;
    [
        Shadow { offset: [0, 1], blur: 2, spread: 0, color: t.shade(77) },
        Shadow { offset: [0, 4], blur: 10, spread: 0, color: t.shade(38) },
        Shadow { offset: [0, 8], blur: 18, spread: 0, color: t.shade(a(0.22 * hover)) },
    ]
}

pub fn paint(p: &Painter, t: &Theme, rect: Rect, c: &CardData, fx: Fx) {
    let rect = rect.translate(vec2(0.0, -LIFT * fx.hover));
    let radius = CornerRadius::same(RADIUS);
    if !fx.floating {
        for s in card_shadows(t, fx.hover) {
            p.add(s.as_shape(rect, radius));
        }
    }
    let base = t.hl_med.lerp_to_gamma(t.hl_high, fx.hover);
    let stroke = Stroke::new(1.0 + fx.select, base.lerp_to_gamma(t.accent_color, fx.select));
    p.rect(rect, radius, t.overlay, stroke, StrokeKind::Inside);
    if fx.focus > 0.0 {
        let ring = Stroke::new(2.0, Theme::tint(t.accent_color, fx.focus));
        p.rect_stroke(rect.expand(3.0), CornerRadius::same(RADIUS + 3), ring, StrokeKind::Inside);
    }
    let inner = Rect::from_min_max(rect.min + vec2(PAD_X, PAD_Y), rect.max - vec2(PAD_X, PAD_Y));
    let x0 = inner.min.x;

    // Top: status circle on the first title line, the title indented after it.
    parts::status_icon(p, t, pos2(x0 + 7.0, inner.min.y + LINE_H / 2.0), c.status, 14.0);
    let g = title_galley(p, t, &c.issue.title, inner.width() - INDENT, fx.select > 0.5);
    p.galley(pos2(x0 + INDENT, inner.min.y), g, t.text);

    // Bottom row: date, priority and labels on the left; dev stage icon and assignee on the right.
    let mid = inner.max.y - ROW_H / 2.0;
    let mut right = inner.max.x;
    let at = pos2(right - AVATAR / 2.0, mid);
    match &c.assignee {
        Some(w) => parts::avatar_photo(p, p.ctx(), t, at, AVATAR, w.avatar_url, Some((w.initials.as_str(), w.color))),
        None => parts::avatar(p, t, at, AVATAR, None),
    }
    right -= AVATAR + ITEM_GAP;
    if let Some(d) = c.dev {
        let icon = if matches!(d.stage, DevStage::Local | DevStage::Merged) { Icon::Branch } else { Icon::Mr };
        parts::icon(p, pos2(right - 7.0, mid), icon, 14.0, t.stage(d.stage));
        right -= 14.0 + ITEM_GAP;
    }
    let label = created_label(c.issue.created_at, Utc::now());
    let date = parts::text(p, pos2(x0, mid), Align2::LEFT_CENTER, &label, W::Medium, 11.0, t.muted);
    let mut x = date.max.x + ITEM_GAP;
    parts::priority_icon(p, t, pos2(x, mid - 7.0), c.issue.priority);
    x += 14.0 + ITEM_GAP;
    let (shown, rest) = label_row(p, &c.labels, right - x);
    for (name, color) in shown {
        x = parts::tag(p, t, pos2(x, mid), &name, color, None, false).max.x + TAG_GAP;
    }
    let more = format!("+{rest}");
    if rest > 0 && x + parts::tag_size(p, &more, false, false).x <= right {
        parts::tag(p, t, pos2(x, mid), &more, t.subtle, None, false);
    }
}

/// Dashed outline where the dragged card was.
pub fn paint_placeholder(p: &Painter, t: &Theme, rect: Rect) {
    p.rect_filled(rect, CornerRadius::same(RADIUS), Theme::tint(t.base, 0.5));
    parts::dashed_rect(p, rect, RADIUS as f32, Stroke::new(1.0, t.hl_high), 4.0, 4.0);
}

/// Dashed accent slot where the dragged card lands.
pub fn paint_drop_slot(p: &Painter, t: &Theme, rect: Rect) {
    p.rect_filled(rect, CornerRadius::same(RADIUS), Theme::tint(t.accent_color, 0.08));
    parts::dashed_rect(p, rect, RADIUS as f32, Stroke::new(1.5, t.accent_color), 5.0, 4.0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, TimeZone};

    #[test]
    fn initials_from_names() {
        assert_eq!(initials("Kaarel"), "KA");
        assert_eq!(initials("Mari Tamm"), "MT");
        assert_eq!(initials("andres pärn"), "AP");
        assert_eq!(initials(""), "?");
    }

    #[test]
    fn created_labels() {
        let now = Local.with_ymd_and_hms(2026, 11, 17, 12, 0, 0).unwrap().with_timezone(&Utc);
        assert_eq!(created_label(now - Duration::hours(2), now), "Today");
        assert_eq!(created_label(now - Duration::hours(26), now), "Yesterday");
        assert_eq!(created_label(now - Duration::days(5), now), "12 Nov");
        assert_eq!(created_label(now - Duration::days(400), now), "13 Oct 2025");
    }

    #[test]
    fn ellipsize_cuts_to_fit() {
        assert_eq!(ellipsize("abc", |s| s.chars().count() <= 3), "abc");
        assert_eq!(ellipsize("abcdef", |s| s.chars().count() <= 4), "abc…");
        assert_eq!(ellipsize("abc", |_| false), "");
    }
}
