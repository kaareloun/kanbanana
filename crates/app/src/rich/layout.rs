//! Text layout for the editor. Every source character stays in the galley, so the
//! TextEdit cursor maps 1:1 to the markdown. Syntax marks get a tiny transparent font, so
//! they take no space. Leading space makes room for the bullets and checkboxes that
//! `overlay` paints.

use std::ops::Range;

use eframe::egui::{
    Align, Color32, FontId, Stroke,
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
pub const RULE_H: f32 = 17.0;
/// Space between a table cell border and its text.
pub const CELL_PAD_X: f32 = 10.0;
pub const CELL_PAD_Y: f32 = 6.0;
/// Height of the hidden separator line between a table header and its body.
pub const TABLE_SEP_H: f32 = 1.0;

/// Measures the width of a text in a font. The layout uses it to align table columns.
pub type Measure<'a> = &'a dyn Fn(&str, &FontId) -> f32;

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
        Block::Heading(l) => {
            let (size, line) = look.h[(l as usize).clamp(1, 3) - 1];
            Base { w: W::Bold, size, color: t.text, line }
        }
        Block::Check(true) => text(t.subtle),
        Block::Quote => text(t.subtle),
        Block::Code | Block::Fence => {
            Base { w: W::Mono, size: look.size - CODE_SIZE_DELTA, color: t.text, line: CODE_LINE }
        }
        Block::TableRow(header) => Base {
            w: if header { W::SemiBold } else { W::Regular },
            size: look.size,
            color: t.text,
            line: look.line + 2.0 * CELL_PAD_Y,
        },
        _ => text(t.text),
    }
}

/// Left space before the text of a line.
pub fn lead(line: &Line) -> f32 {
    let level = line.depth.saturating_sub(1) as f32 * LIST_INDENT;
    match line.block {
        Block::Bullet | Block::Numbered(_) => level + LIST_INDENT,
        Block::Check(_) => level + CHECK_INDENT,
        Block::Para if line.depth > 0 => level + LIST_INDENT,
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
            Block::Rule => RULE_H,
            Block::TableSep => TABLE_SEP_H,
            Block::Blank if i > 0 && !blank(i - 1) && !blank(i + 1) => {
                let heading = matches!(lines[i + 1].block, Block::Heading(_));
                if heading { HEADING_GAP } else { GAP_LINE }
            }
            b => base(b, look, t).line,
        })
        .collect()
}

/// The layout job for `text`. `lines` is `parse(text)`.
pub fn job(
    text: &str,
    lines: &[Line],
    t: &Theme,
    look: &Look,
    wrap: f32,
    image_h: &dyn Fn(&Image) -> f32,
    measure: Measure,
) -> LayoutJob {
    job_lines(text, lines, 0..lines.len(), t, look, wrap, image_h, measure)
}

/// The layout job for the lines `part` of `lines`. Its text is the source text of those
/// lines. Blank line heights look at the lines around the part.
#[allow(clippy::too_many_arguments)]
pub fn job_lines(
    text: &str,
    lines: &[Line],
    part: Range<usize>,
    t: &Theme,
    look: &Look,
    wrap: f32,
    image_h: &dyn Fn(&Image) -> f32,
    measure: Measure,
) -> LayoutJob {
    let mut job = LayoutJob::default();
    let heights = line_heights(lines, look, t, image_h);
    let mut cols: Option<Vec<f32>> = None;
    let tables = super::parse::tables(lines);
    for i in part.clone() {
        let line = &lines[i];
        let b = base(line.block, look, t);
        let h = heights[i];
        if line.block == Block::TableRow(true) {
            let group = tables.iter().find(|g| g.start == i).cloned().unwrap_or(i..i + 1);
            cols = table_columns(text, lines, group, t, look, wrap, measure);
        }
        if line.all_hidden() {
            job.append(&text[line.range.clone()], 0.0, hidden(h));
        } else if let (Block::TableRow(_), Some(bounds)) = (line.block, &cols) {
            table_row(&mut job, text, line, t, look, h, bounds, measure);
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
        if !line.is_table() {
            cols = None;
        }
        if i + 1 < part.end {
            // An empty line takes the height of its break, so the break uses the line height.
            job.append("\n", 0.0, hidden(h));
        }
    }
    job.wrap.max_width = wrap;
    job
}

/// Spans of a table row that belong to cell `k`.
fn cell_spans<'a>(line: &'a Line, k: usize) -> impl Iterator<Item = (usize, &'a super::parse::Span)> {
    let c = line.cells[k].clone();
    line.spans.iter().enumerate().filter(move |(_, s)| s.range.start >= c.start && s.range.end <= c.end)
}

/// Width of the text of cell `k`, as the layout draws it.
fn cell_width(text: &str, line: &Line, k: usize, t: &Theme, look: &Look, measure: Measure) -> f32 {
    let b = base(line.block, look, t);
    cell_spans(line, k)
        .map(|(i, s)| {
            let (f, gap) = span_format(t, &b, b.line, &s.mark, line, i);
            let w = if f.color == Color32::TRANSPARENT { 0.0 } else { measure(&text[s.range.clone()], &f.font_id) };
            gap + w
        })
        .sum()
}

/// The x of each column border of the table `group` (line indices), from its left edge.
/// The first is 0 and the last is the table width. None when the table is wider than
/// `wrap`: its rows then wrap like text and show their pipes dimmed.
pub fn table_columns(
    text: &str,
    lines: &[Line],
    group: Range<usize>,
    t: &Theme,
    look: &Look,
    wrap: f32,
    measure: Measure,
) -> Option<Vec<f32>> {
    let mut widths: Vec<f32> = Vec::new();
    for line in &lines[group] {
        for k in 0..line.cells.len() {
            let w = cell_width(text, line, k, t, look, measure);
            if widths.len() <= k {
                widths.push(0.0);
            }
            widths[k] = widths[k].max(w.ceil());
        }
    }
    let mut bounds = vec![0.0];
    for w in widths {
        bounds.push(bounds[bounds.len() - 1] + w + 2.0 * CELL_PAD_X);
    }
    (bounds[bounds.len() - 1] <= wrap - 1.0).then_some(bounds)
}

/// Appends an aligned table row. The hidden pipes take the space up to each border and
/// give the row its padded height. The cell text has the normal line height, centered
/// in the row, so strikethrough and code backgrounds sit on the text.
#[allow(clippy::too_many_arguments)]
fn table_row(job: &mut LayoutJob, text: &str, line: &Line, t: &Theme, look: &Look, h: f32, bounds: &[f32], measure: Measure) {
    let b = base(line.block, look, t);
    let centered = |mut f: TextFormat| {
        f.valign = Align::Center;
        f
    };
    let mut x = 0.0;
    let mut at = line.range.start;
    for k in 0..line.cells.len() {
        let cell = line.cells[k].clone();
        let left = bounds.get(k).copied().unwrap_or(x);
        job.append(&text[at..cell.start], (left - x).max(0.0), hidden(h));
        let mut lead = CELL_PAD_X;
        let mut any = false;
        for (i, s) in cell_spans(line, k) {
            let (f, gap) = span_format(t, &b, look.line, &s.mark, line, i);
            job.append(&text[s.range.clone()], lead + gap, centered(f));
            lead = 0.0;
            any = true;
        }
        if !any {
            job.append("", lead, centered(fmt(b.w, b.size, b.color, look.line)));
        }
        x = left.max(x) + CELL_PAD_X + cell_width(text, line, k, t, look, measure);
        at = cell.end;
    }
    let right = bounds.last().copied().unwrap_or(x);
    job.append(&text[at..line.range.end], (right - x).max(0.0), hidden(h));
}

/// The layout job of one table cell, for the read-only table. Its text is the cell text.
pub fn cell_job(text: &str, line: &Line, k: usize, t: &Theme, look: &Look, wrap: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    let b = base(line.block, look, t);
    let mut any = false;
    for (i, s) in cell_spans(line, k) {
        let (f, gap) = span_format(t, &b, look.line, &s.mark, line, i);
        job.append(&text[s.range.clone()], gap, f);
        any = true;
    }
    if !any {
        job.append("", 0.0, fmt(b.w, b.size, b.color, look.line));
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
        // No italic font: Medium weight stands in for it.
        Mark::Italic => (fmt(if b.w == W::Regular { W::Medium } else { W::Bold }, b.size, b.color, h), 0.0),
        Mark::Strike => {
            let mut f = fmt(b.w, b.size, b.color, h);
            f.strikethrough = Stroke::new(1.0, b.color);
            (f, 0.0)
        }
        Mark::Code => (fmt(W::Mono, b.size - CODE_SIZE_DELTA, t.foam, h), 0.0),
        Mark::Link { .. } | Mark::Mention => (fmt(b.w, b.size, t.accent_color, h), 0.0),
        // Pipes show only when a table is too wide to align. Then they mark the cells.
        Mark::Pipe => (fmt(b.w, b.size, t.hl_high, h), 0.0),
        Mark::Break => (hidden(h), b.size * 0.3),
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
        let s = "When a **partial refund**, x.\n\n## Steps\n1. Pay\n- [x] Done `a`\n![p.png](u)\n```\ncode\n```\n\
                 | a | b |\n| - | - |\n| <https://x> |  |\n---\na<br>b @mt\n";
        let lines = parse(s, true);
        for wrap in [500.0, 20.0] {
            let job = job(s, &lines, &Theme::MAIN, &super::super::DESCRIPTION, wrap, &|_| 100.0, &chars);
            assert_eq!(job.text, s);
        }
    }

    /// A fake font: every char is half the font size wide.
    fn chars(s: &str, f: &FontId) -> f32 {
        s.chars().count() as f32 * f.size * 0.5
    }

    #[test]
    fn aligns_table_columns() {
        let s = "| a | bb |\n| - | - |\n| ccc | d |";
        let lines = parse(s, true);
        let look = super::super::DESCRIPTION;
        let t = Theme::MAIN;
        let p = 2.0 * CELL_PAD_X;
        assert_eq!(table_columns(s, &lines, 0..3, &t, &look, 500.0, &chars), Some(vec![0.0, 21.0 + p, 35.0 + 2.0 * p]));
        assert_eq!(table_columns(s, &lines, 0..3, &t, &look, 50.0, &chars), None);
        let job = job(s, &lines, &t, &look, 500.0, &|_| 0.0, &chars);
        let lead = |needle: &str| {
            job.sections.iter().find(|x| &job.text[x.byte_range.start.0..x.byte_range.end.0] == needle).map(|x| x.leading_space)
        };
        assert_eq!(lead("a"), Some(CELL_PAD_X));
        // After "a" (7 wide) the pipe fills up to the second border.
        assert_eq!(lead(" | "), Some(21.0 + p - CELL_PAD_X - 7.0));
        assert_eq!(lead("ccc"), Some(CELL_PAD_X));
    }

    #[test]
    fn wide_tables_show_dim_pipes() {
        let s = "| a | b |\n| - | - |";
        let lines = parse(s, true);
        let job = job(s, &lines, &Theme::MAIN, &super::super::DESCRIPTION, 10.0, &|_| 0.0, &chars);
        let pipe = job.sections.iter().find(|x| &job.text[x.byte_range.start.0..x.byte_range.end.0] == "| ").map(|x| x.format.color);
        assert_eq!(pipe, Some(Theme::MAIN.hl_high));
    }

    #[test]
    fn comment_headings_are_smaller() {
        let t = Theme::MAIN;
        assert_eq!(base(Block::Heading(1), &super::super::DESCRIPTION, &t).size, 24.0);
        assert!(base(Block::Heading(1), &super::super::COMMENT, &t).size < 24.0);
        assert_eq!(base(Block::TableRow(true), &super::super::COMMENT, &t).w, W::SemiBold);
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
        let lines = parse("- a\n  - b\n- [ ] c\n> d\ne\n- f\n  g", true);
        let leads: Vec<f32> = lines.iter().map(lead).collect();
        assert_eq!(leads, vec![LIST_INDENT, 2.0 * LIST_INDENT, CHECK_INDENT, QUOTE_INDENT, 0.0, LIST_INDENT, LIST_INDENT]);
    }
}
