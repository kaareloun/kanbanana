//! Text layout for the editor. Every source character stays in the galley, so the
//! TextEdit cursor maps 1:1 to the markdown. Syntax marks get a tiny transparent font, so
//! they take no space. Leading space makes room for the bullets and checkboxes that
//! `overlay` paints.

use eframe::egui::{
    Color32, Stroke,
    text::{LayoutJob, TextFormat},
};

use super::Look;
use super::parse::{Block, Image, Line, Mark};
use crate::theme::{Theme, W, font};

/// Font size of hidden marks. Small enough to take no visible space.
const HIDDEN: f32 = 0.01;
/// Indent of one list level.
pub const LIST_INDENT: f32 = 22.0;
/// Text indent of a checklist item, from the start of its level.
pub const CHECK_INDENT: f32 = 26.0;
pub const QUOTE_INDENT: f32 = 14.0;
pub const CODE_PAD: f32 = 12.0;
pub const FENCE_H: f32 = 10.0;
/// Space around inline code, inside its background.
pub const CODE_GAP: f32 = 8.0;
pub const CODE_SIZE_DELTA: f32 = 1.0;
const CODE_LINE: f32 = 20.0;
/// A blank line between two paragraphs.
const GAP_LINE: f32 = 12.0;
/// A blank line before a heading.
const HEADING_GAP: f32 = 20.0;

/// Font, size, color and line height of a block.
#[derive(Clone, Copy, Debug)]
pub struct Base {
    pub w: W,
    pub size: f32,
    pub color: Color32,
    pub line: f32,
}

pub fn base(block: Block, look: &Look, t: &Theme) -> Base {
    let text = |color| Base { w: W::Regular, size: look.size, color, line: look.line };
    match block {
        Block::Heading(1) => Base { w: W::Bold, size: 24.0, color: t.text, line: 34.0 },
        Block::Heading(2) => Base { w: W::Bold, size: 19.0, color: t.text, line: 29.0 },
        Block::Heading(_) => Base { w: W::Bold, size: 16.0, color: t.text, line: 26.0 },
        Block::Check(true) => text(t.subtle),
        Block::Quote => text(t.subtle),
        Block::Code | Block::Fence => {
            Base { w: W::Mono, size: look.size - CODE_SIZE_DELTA, color: t.text, line: CODE_LINE }
        }
        _ => text(t.text),
    }
}

/// Left space before the text of a line.
pub fn lead(line: &Line) -> f32 {
    let level = line.depth.saturating_sub(1) as f32 * LIST_INDENT;
    match line.block {
        Block::Bullet | Block::Numbered(_) => level + LIST_INDENT,
        Block::Check(_) => level + CHECK_INDENT,
        Block::Quote => QUOTE_INDENT,
        Block::Code => CODE_PAD,
        _ => 0.0,
    }
}

fn fmt(w: W, size: f32, color: Color32, line: f32) -> TextFormat {
    TextFormat { font_id: font(w, size), color, line_height: Some(line), ..Default::default() }
}

fn hidden(line: f32) -> TextFormat {
    fmt(W::Regular, HIDDEN, Color32::TRANSPARENT, line)
}

/// Line height of each line. A single blank line between two text lines is a short
/// paragraph gap. Other blank lines are full lines, so the caret on them has full height.
pub fn line_heights(lines: &[Line], look: &Look, t: &Theme, image_h: &dyn Fn(&Image) -> f32) -> Vec<f32> {
    let blank = |i: usize| lines.get(i).is_none_or(|l| l.block == Block::Blank);
    (0..lines.len())
        .map(|i| match lines[i].block {
            Block::Image => lines[i].image.as_ref().map_or(look.line, image_h),
            Block::Fence => FENCE_H,
            Block::Blank if i > 0 && !blank(i - 1) && !blank(i + 1) => {
                let heading = matches!(lines[i + 1].block, Block::Heading(_));
                if heading { HEADING_GAP } else { GAP_LINE }
            }
            b => base(b, look, t).line,
        })
        .collect()
}

/// The layout job for `text`. `lines` is `parse(text)`.
pub fn job(text: &str, lines: &[Line], t: &Theme, look: &Look, wrap: f32, image_h: &dyn Fn(&Image) -> f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    let heights = line_heights(lines, look, t, image_h);
    for (i, line) in lines.iter().enumerate() {
        let b = base(line.block, look, t);
        let h = heights[i];
        if line.all_hidden() {
            job.append(&text[line.range.clone()], 0.0, hidden(h));
        } else {
            let mut lead = lead(line);
            if !line.marker.is_empty() {
                job.append(&text[line.marker.clone()], lead, hidden(h));
                lead = 0.0;
            }
            if line.spans.is_empty() {
                job.append("", lead, fmt(b.w, b.size, b.color, h));
            }
            for (k, s) in line.spans.iter().enumerate() {
                let (f, gap) = span_format(t, &b, h, &s.mark, line, k);
                job.append(&text[s.range.clone()], lead + gap, f);
                lead = 0.0;
            }
        }
        if i + 1 < lines.len() {
            // An empty line takes the height of its break, so the break uses the line height.
            job.append("\n", 0.0, hidden(h));
        }
    }
    job.wrap.max_width = wrap;
    job
}

/// Format of one inline run, and the space before it.
fn span_format(t: &Theme, b: &Base, h: f32, mark: &Mark, line: &Line, k: usize) -> (TextFormat, f32) {
    let code_mark = |i: usize| line.spans.get(i).is_some_and(|s| s.mark == Mark::Code);
    match mark {
        Mark::Text => (fmt(b.w, b.size, b.color, h), 0.0),
        Mark::Bold => (fmt(W::Bold, b.size, b.color, h), 0.0),
        Mark::Italic => (fmt(W::Medium, b.size, b.color, h), 0.0),
        Mark::Strike => {
            let mut f = fmt(b.w, b.size, b.color, h);
            f.strikethrough = Stroke::new(1.0, b.color);
            (f, 0.0)
        }
        Mark::Code => (fmt(W::Mono, b.size - CODE_SIZE_DELTA, t.foam, h), 0.0),
        Mark::Link { .. } => (fmt(b.w, b.size, t.accent_color, h), 0.0),
        // The backticks around inline code make room for its background.
        Mark::Hidden if code_mark(k + 1) || (k > 0 && code_mark(k - 1)) => (hidden(h), CODE_GAP),
        Mark::Hidden => (hidden(h), 0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::super::parse::parse;
    use super::*;

    #[test]
    fn job_keeps_every_character() {
        let s = "When a **partial refund**, x.\n\n## Steps\n1. Pay\n- [x] Done `a`\n![p.png](u)\n```\ncode\n```\n";
        let lines = parse(s, true);
        let job = job(s, &lines, &Theme::MAIN, &super::super::DESCRIPTION, 500.0, &|_| 100.0);
        assert_eq!(job.text, s);
    }

    #[test]
    fn heights_follow_blocks() {
        let s = "a\n\nb\n\n\n![p](u)\n```";
        let lines = parse(s, true);
        let look = super::super::DESCRIPTION;
        let h = line_heights(&lines, &look, &Theme::MAIN, &|_| 200.0);
        assert_eq!(h, vec![look.line, GAP_LINE, look.line, look.line, look.line, 200.0, FENCE_H]);
    }

    #[test]
    fn leads_make_room_for_markers() {
        let lines = parse("- a\n  - b\n- [ ] c\n> d\ne", true);
        let leads: Vec<f32> = lines.iter().map(lead).collect();
        assert_eq!(leads, vec![LIST_INDENT, 2.0 * LIST_INDENT, CHECK_INDENT, QUOTE_INDENT, 0.0]);
    }
}
