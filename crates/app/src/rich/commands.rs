//! Pure markdown edits for the editor keys, shortcuts and toolbar.
//! Offsets are byte offsets into the text. The editor converts its char cursor first.

use std::ops::Range;

use super::parse::{Block, Hide, Line, hidden, line_at};

/// Result of an edit: the new text and the new selection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edit {
    pub text: String,
    pub sel: Range<usize>,
}

impl Edit {
    fn caret(text: String, at: usize) -> Edit {
        Edit { text, sel: at..at }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListKind {
    Bullet,
    Numbered,
    Check,
}

fn sorted(sel: &Range<usize>) -> (usize, usize) {
    (sel.start.min(sel.end), sel.start.max(sel.end))
}

fn splice(text: &str, r: Range<usize>, with: &str) -> String {
    let mut s = String::with_capacity(text.len() + with.len());
    s.push_str(&text[..r.start]);
    s.push_str(with);
    s.push_str(&text[r.end..]);
    s
}

/// Wraps the selection in `mark` (`**`, `~~` or a backtick). Unwraps it when the marks
/// are already around the selection.
pub fn wrap(text: &str, sel: Range<usize>, mark: &str) -> Edit {
    let (a, b) = sorted(&sel);
    let n = mark.len();
    if a >= n && text[..a].ends_with(mark) && text[b..].starts_with(mark) {
        let s = format!("{}{}{}", &text[..a - n], &text[a..b], &text[b + n..]);
        return Edit { text: s, sel: a - n..b - n };
    }
    let s = format!("{}{mark}{}{mark}{}", &text[..a], &text[a..b], &text[b..]);
    Edit { text: s, sel: a + n..b + n }
}

/// Inline code for a one-line selection, a fenced block when it spans lines.
pub fn code(text: &str, sel: Range<usize>) -> Edit {
    let (a, b) = sorted(&sel);
    if !text[a..b].contains('\n') {
        return wrap(text, a..b, "`");
    }
    let before = if a == 0 || text[..a].ends_with('\n') { "" } else { "\n" };
    let after = if b == text.len() || text[b..].starts_with('\n') { "" } else { "\n" };
    let open = format!("{before}```\n");
    let s = format!("{}{open}{}\n```{after}{}", &text[..a], &text[a..b], &text[b..]);
    Edit { text: s, sel: a + open.len()..b + open.len() }
}

/// Makes the selection a link to `url`. Without a selection it links the word at the
/// caret, or inserts the url as its own text. The caret goes after the link.
pub fn link(text: &str, sel: Range<usize>, url: &str) -> Edit {
    let (mut a, mut b) = sorted(&sel);
    if a == b {
        let w = word_at(text, a);
        (a, b) = (w.start, w.end);
    }
    let label = if a == b { url } else { &text[a..b] };
    let md = format!("[{label}]({url})");
    let at = a + md.len();
    Edit::caret(splice(text, a..b, &md), at)
}

/// The word around byte `at`: letters, digits, `_` and `-`.
pub fn word_at(text: &str, at: usize) -> Range<usize> {
    let is_word = |c: char| c.is_alphanumeric() || c == '_' || c == '-';
    let start = text[..at].char_indices().rev().take_while(|(_, c)| is_word(*c)).last().map_or(at, |(i, _)| i);
    let end = text[at..].char_indices().find(|(_, c)| !is_word(*c)).map_or(text.len(), |(i, _)| at + i);
    start..end
}

/// Makes the caret line a heading of `level`, or a paragraph when it already is one.
pub fn heading(text: &str, cursor: usize, level: u8) -> Edit {
    let start = line_start(text, cursor);
    let line = &text[start..line_end(text, cursor)];
    let hashes = line.bytes().take_while(|c| *c == b'#').count();
    let old = if hashes > 0 && line[hashes..].starts_with(' ') { hashes + 1 } else { 0 };
    let new = if old == level as usize + 1 { String::new() } else { format!("{} ", "#".repeat(level as usize)) };
    let s = splice(text, start..start + old, &new);
    let at = (cursor + new.len()).saturating_sub(old).max(start + new.len());
    Edit::caret(s, at)
}

/// Puts a list prefix on every line of the selection. Removes it when all lines have it.
pub fn prefix_lines(text: &str, sel: Range<usize>, kind: ListKind) -> Edit {
    let (a, b) = sorted(&sel);
    let start = line_start(text, a);
    let end = line_end(text, b);
    let lines: Vec<&str> = text[start..end].split('\n').collect();
    let all = lines.iter().all(|l| list_prefix(l).is_some_and(|(k, _)| k == kind));
    let mut out = Vec::with_capacity(lines.len());
    let mut n = 0;
    for l in &lines {
        let body = match list_prefix(l) {
            Some((_, len)) => &l[len..],
            None => heading_body(l),
        };
        if all {
            out.push(body.to_string());
        } else {
            n += 1;
            let p = match kind {
                ListKind::Bullet => "- ".to_string(),
                ListKind::Numbered => format!("{n}. "),
                ListKind::Check => "- [ ] ".to_string(),
            };
            out.push(format!("{p}{body}"));
        }
    }
    let joined = out.join("\n");
    let s = format!("{}{joined}{}", &text[..start], &text[end..]);
    // A caret without a selection goes to the end of its line, ready to type.
    let sel = if a == b && lines.len() == 1 {
        let at = start + joined.len();
        at..at
    } else {
        start..start + joined.len()
    };
    Edit { text: s, sel }
}

fn heading_body(l: &str) -> &str {
    let hashes = l.bytes().take_while(|c| *c == b'#').count();
    if hashes > 0 && l[hashes..].starts_with(' ') { &l[hashes + 1..] } else { l }
}

/// The list prefix of a line and its byte length, after leading spaces.
pub fn list_prefix(line: &str) -> Option<(ListKind, usize)> {
    let (block, len, _) = super::parse::block_marker(line, false);
    let kind = match block {
        Block::Bullet => ListKind::Bullet,
        Block::Numbered(_) => ListKind::Numbered,
        Block::Check(_) => ListKind::Check,
        _ => return None,
    };
    Some((kind, len))
}

/// Enter in a list item or quote: continues it on a new line. Enter on an empty item
/// ends the list. Returns None when the editor inserts a plain line break.
pub fn enter(text: &str, lines: &[Line], cursor: usize) -> Option<Edit> {
    let li = line_at(lines, cursor)?;
    let l = &lines[li];
    if let Block::TableRow(header) = l.block {
        return Some(table_enter(text, lines, li, header));
    }
    if !(l.is_list() || l.block == Block::Quote) || cursor < l.marker.end {
        return None;
    }
    if text[l.marker.end..l.range.end].trim().is_empty() {
        return Some(Edit::caret(splice(text, l.marker.clone(), ""), l.marker.start));
    }
    let raw = &text[l.range.clone()];
    let indent = &raw[..raw.len() - raw.trim_start_matches(' ').len()];
    let next = match l.block {
        Block::Numbered(n) => {
            let delim = if text[..l.marker.end].ends_with(") ") { ")" } else { "." };
            format!("{indent}{}{delim} ", n + 1)
        }
        Block::Check(_) => format!("{indent}- [ ] "),
        _ => text[l.marker.clone()].to_string(),
    };
    let insert = format!("\n{next}");
    Some(Edit::caret(splice(text, cursor..cursor, &insert), cursor + insert.len()))
}

/// Enter in a table row adds an empty row below it, or below the separator for the
/// header, and puts the caret in its first cell. Splitting a row would break the table.
fn table_enter(text: &str, lines: &[Line], li: usize, header: bool) -> Edit {
    let after = if header && lines.get(li + 1).is_some_and(|l| l.block == Block::TableSep) { li + 1 } else { li };
    let at = lines[after].range.end;
    let row = format!("\n|{}", "  |".repeat(lines[li].cells.len().max(1)));
    Edit::caret(splice(text, at..at, &row), at + 3)
}

/// Backspace without a selection. It removes the visible character before the caret and
/// keeps the hidden marks around it. At the start of a block it removes the marker.
pub fn backspace(text: &str, lines: &[Line], cursor: usize) -> Option<Edit> {
    let hid = hidden(lines);
    let p = skip_back(&hid, cursor);
    let li = line_at(lines, p)?;
    let l = &lines[li];
    if !l.marker.is_empty() && !l.all_hidden() && p == l.marker.end {
        return Some(Edit::caret(splice(text, l.marker.clone(), ""), l.marker.start));
    }
    if p == l.range.start && li > 0 {
        let above = &lines[li - 1];
        match above.block {
            Block::Image => {
                let r = above.range.start..above.range.end + 1;
                return Some(Edit::caret(splice(text, r, ""), above.range.start));
            }
            // A fence never joins a line, or the code block breaks. Backspace after the
            // block goes into it. At the start of the block it does nothing.
            Block::Fence if l.block == Block::Code => return Some(Edit::caret(text.to_string(), p)),
            Block::Fence if li >= 2 => return Some(Edit::caret(text.to_string(), lines[li - 2].range.end)),
            Block::Rule => {
                let r = above.range.start..above.range.end + 1;
                return Some(Edit::caret(splice(text, r, ""), above.range.start));
            }
            // Joining two table rows breaks the table. The caret goes to the row above.
            Block::TableSep if li >= 2 => return Some(Edit::caret(text.to_string(), table_end(&lines[li - 2]))),
            Block::TableRow(_) if l.block == Block::TableRow(false) => {
                return Some(Edit::caret(text.to_string(), table_end(above)));
            }
            _ => {}
        }
    }
    if p == 0 {
        return None;
    }
    let mut start = p - text[..p].chars().next_back().map_or(1, char::len_utf8);
    if escaped(text, &hid, start, p) {
        start -= 1;
    }
    Some(Edit::caret(splice(text, start..p, ""), start))
}

/// Delete without a selection. Like `backspace`, but forward. At a line end it joins the
/// next line without its marker, so no raw marker shows up mid line.
pub fn delete(text: &str, lines: &[Line], cursor: usize) -> Option<Edit> {
    let hid = hidden(lines);
    let mut p = cursor;
    while let Some((r, _)) = hid.iter().find(|(r, k)| *k == Hide::Inline && r.start == p && r.end > p) {
        p = r.end;
    }
    let li = line_at(lines, p)?;
    let l = &lines[li];
    if p == l.range.end {
        let below = lines.get(li + 1)?;
        return match below.block {
            Block::Fence => None,
            Block::TableSep | Block::TableRow(_) if l.is_table() => Some(Edit::caret(text.to_string(), cursor)),
            Block::Image => {
                let r = below.range.start - 1..below.range.end;
                Some(Edit::caret(splice(text, r, ""), cursor))
            }
            _ => Some(Edit::caret(splice(text, p..below.marker.end, ""), cursor)),
        };
    }
    let end = p + text[p..].chars().next().map_or(1, char::len_utf8);
    let start = if escaped(text, &hid, p, end) { p - 1 } else { p };
    let caret = cursor.min(start);
    Some(Edit::caret(splice(text, start..end, ""), caret))
}

/// The end of the last cell text of a table row.
fn table_end(l: &Line) -> usize {
    l.cells.last().map_or(l.range.end, |c| c.end)
}

/// Moves `p` left over inline marks that end at it.
fn skip_back(hid: &[(Range<usize>, Hide)], mut p: usize) -> usize {
    while let Some((r, _)) = hid.iter().find(|(r, k)| *k == Hide::Inline && r.end == p && r.start < p) {
        p = r.start;
    }
    p
}

/// True when `text[a..b]` is a character escaped by a hidden backslash.
fn escaped(text: &str, hid: &[(Range<usize>, Hide)], a: usize, b: usize) -> bool {
    a > 0 && b == a + 1 && hid.iter().any(|(r, k)| *k == Hide::Inline && *r == (a - 1..a) && &text[r.clone()] == "\\")
}

/// Markdown shortcuts that need a rewrite after typing: `[] ` makes a checklist item and
/// ``` makes a code block. `#`, `-`, `1.` and `>` need none, the parser shows them as blocks.
pub fn auto_block(text: &str, cursor: usize) -> Option<Edit> {
    let start = line_start(text, cursor);
    if cursor != line_end(text, cursor) {
        return None;
    }
    let line = &text[start..cursor];
    if line == "[] " || line == "[ ] " || line == "- [] " {
        return Some(Edit::caret(splice(text, start..cursor, "- [ ] "), start + 6));
    }
    let fences_before = text[..start].split('\n').filter(|l| l.trim().starts_with("```")).count();
    let fences_after = text[cursor..].split('\n').filter(|l| l.trim().starts_with("```")).count();
    if line == "```" && fences_before % 2 == 0 && fences_after == 0 {
        return Some(Edit::caret(splice(text, start..cursor, "```\n\n```"), start + 4));
    }
    None
}

/// Flips `[ ]` and `[x]` in the marker of a checklist line.
pub fn toggle_check(text: &str, line: &Line) -> Option<String> {
    let m = &text[line.marker.clone()];
    let (i, with) = match m.find("[ ]") {
        Some(i) => (i, "[x]"),
        None => (m.find("[x]").or_else(|| m.find("[X]"))?, "[ ]"),
    };
    let at = line.marker.start + i;
    Some(splice(text, at..at + 3, with))
}

/// Removes empty mark pairs such as `****`, which the shortcuts leave when the user
/// types nothing into them. A pair with the caret between its marks stays.
pub fn drop_empty_marks(text: &str, lines: &[Line], cursor: usize) -> Option<Edit> {
    const PAIRS: [&str; 6] = ["**", "__", "~~", "`", "*", "_"];
    for l in lines {
        for w in l.spans.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            let m = &text[a.range.clone()];
            let pair = a.range.end == b.range.start && m == &text[b.range.clone()] && PAIRS.contains(&m);
            if a.mark == super::parse::Mark::Hidden && b.mark == super::parse::Mark::Hidden && pair && cursor != a.range.end {
                let r = a.range.start..b.range.end;
                let caret = if cursor >= r.end { cursor - r.len() } else { cursor.min(r.start) };
                return Some(Edit::caret(splice(text, r, ""), caret));
            }
        }
    }
    None
}

fn line_start(text: &str, i: usize) -> usize {
    text[..i].rfind('\n').map_or(0, |p| p + 1)
}

fn line_end(text: &str, i: usize) -> usize {
    text[i..].find('\n').map_or(text.len(), |p| i + p)
}

pub fn char_to_byte(text: &str, c: usize) -> usize {
    text.char_indices().nth(c).map_or(text.len(), |(b, _)| b)
}

pub fn byte_to_char(text: &str, b: usize) -> usize {
    text[..b.min(text.len())].chars().count()
}

#[cfg(test)]
mod tests {
    use super::super::parse::parse;
    use super::*;

    fn run(f: fn(&str, &[Line], usize) -> Option<Edit>, t: &str, at: usize) -> Option<(String, usize)> {
        f(t, &parse(t, true), at).map(|e| (e.text, e.sel.start))
    }

    #[test]
    fn wraps_and_unwraps_bold() {
        let e = wrap("a partial refund b", 2..16, "**");
        assert_eq!(e.text, "a **partial refund** b");
        assert_eq!(&e.text[e.sel.clone()], "partial refund");
        assert_eq!(wrap(&e.text, e.sel, "**"), Edit { text: "a partial refund b".into(), sel: 2..16 });
        assert_eq!(wrap("ab", 1..1, "**"), Edit { text: "a****b".into(), sel: 3..3 });
    }

    #[test]
    fn code_is_inline_or_fenced() {
        assert_eq!(code("x y", 2..3).text, "x `y`");
        let e = code("a\nb\nc", 2..5);
        assert_eq!(e.text, "a\n```\nb\nc\n```");
        assert_eq!(&e.text[e.sel], "b\nc");
    }

    #[test]
    fn links_selection_word_or_url() {
        assert_eq!(link("see docs now", 4..8, "https://x"), Edit::caret("see [docs](https://x) now".into(), 21));
        assert_eq!(link("see docs now", 6..6, "u").text, "see [docs](u) now");
        assert_eq!(link("a  b", 2..2, "u").text, "a [u](u) b");
    }

    #[test]
    fn sets_and_clears_headings() {
        assert_eq!(heading("a\nSteps\nb", 4, 2), Edit::caret("a\n## Steps\nb".into(), 7));
        assert_eq!(heading("a\n## Steps\nb", 7, 2), Edit::caret("a\nSteps\nb".into(), 4));
        assert_eq!(heading("## x", 4, 1).text, "# x");
    }

    #[test]
    fn prefixes_and_unprefixes_lines() {
        let e = prefix_lines("one\ntwo\nthree", 0..7, ListKind::Numbered);
        assert_eq!(e.text, "1. one\n2. two\nthree");
        assert_eq!(prefix_lines(&e.text, 0..12, ListKind::Numbered).text, "one\ntwo\nthree");
        assert_eq!(prefix_lines("- a\nb", 0..5, ListKind::Check).text, "- [ ] a\n- [ ] b");
        assert_eq!(prefix_lines("x\ntodo", 3..3, ListKind::Bullet), Edit::caret("x\n- todo".into(), 8));
        assert_eq!(prefix_lines("## h", 0..0, ListKind::Bullet).text, "- h");
    }

    #[test]
    fn enter_continues_lists() {
        assert_eq!(run(enter, "- a", 3), Some(("- a\n- ".into(), 6)));
        assert_eq!(run(enter, "2. a", 4), Some(("2. a\n3. ".into(), 8)));
        assert_eq!(run(enter, "- [x] a", 7), Some(("- [x] a\n- [ ] ".into(), 14)));
        assert_eq!(run(enter, "  * a", 5), Some(("  * a\n  * ".into(), 10)));
        assert_eq!(run(enter, "> q", 3), Some(("> q\n> ".into(), 6)));
        // Splits the item at the caret.
        assert_eq!(run(enter, "- ab", 3), Some(("- a\n- b".into(), 6)));
    }

    #[test]
    fn enter_on_empty_item_ends_the_list() {
        assert_eq!(run(enter, "- a\n- ", 6), Some(("- a\n".into(), 4)));
        assert_eq!(run(enter, "- [ ] ", 6), Some(("".into(), 0)));
        assert_eq!(run(enter, "plain", 5), None);
        assert_eq!(run(enter, "## h", 4), None);
    }

    #[test]
    fn backspace_removes_block_markers() {
        assert_eq!(run(backspace, "x\n- [ ] a", 8), Some(("x\na".into(), 2)));
        assert_eq!(run(backspace, "## Steps", 3), Some(("Steps".into(), 0)));
        assert_eq!(run(backspace, "> q", 2), Some(("q".into(), 0)));
    }

    #[test]
    fn backspace_keeps_hidden_marks() {
        assert_eq!(run(backspace, "a **bc**", 8), Some(("a **b**".into(), 5)));
        assert_eq!(run(backspace, "**b**", 5), Some(("****".into(), 2)));
        assert_eq!(run(backspace, "[docs](u)", 9), Some(("[doc](u)".into(), 4)));
        assert_eq!(run(backspace, r"a \*", 4), Some(("a ".into(), 2)));
        assert_eq!(run(backspace, "ab", 0), None);
    }

    #[test]
    fn backspace_after_image_removes_it() {
        assert_eq!(run(backspace, "a\n![x](u)\nb", 10), Some(("a\nb".into(), 2)));
        assert_eq!(run(backspace, "```\nc\n```\nb", 10), Some(("```\nc\n```\nb".into(), 5)));
        assert_eq!(run(backspace, "a\nb", 2), Some(("ab".into(), 1)));
        assert_eq!(run(backspace, "```\nc\n```", 4), Some(("```\nc\n```".into(), 4)));
    }

    #[test]
    fn delete_joins_without_markers() {
        assert_eq!(run(delete, "a\n- b", 1), Some(("ab".into(), 1)));
        assert_eq!(run(delete, "a\n![x](u)", 1), Some(("a".into(), 1)));
        assert_eq!(run(delete, "**ab**", 0), Some(("**b**".into(), 0)));
        assert_eq!(run(delete, "a", 1), None);
    }

    #[test]
    fn enter_in_a_table_adds_a_row() {
        let t = "| a | b |\n| - | - |\n| 1 | 2 |";
        assert_eq!(run(enter, t, 2), Some(("| a | b |\n| - | - |\n|  |  |\n| 1 | 2 |".into(), 22)));
        assert_eq!(run(enter, t, 24), Some((format!("{t}\n|  |  |"), 32)));
        // The caret lands in the first cell of the new row.
        let (s, at) = run(enter, t, 24).unwrap();
        let lines = parse(&s, true);
        assert_eq!(lines[3].block, Block::TableRow(false));
        assert_eq!(lines[3].cells[0], at..at);
    }

    #[test]
    fn backspace_and_delete_keep_tables_whole() {
        let t = "| a | b |\n| - | - |\n| 1 | 2 |\n| 3 | 4 |";
        // Start of the first body row: the caret goes to the end of the header.
        assert_eq!(run(backspace, t, 22), Some((t.to_string(), 7)));
        // Start of the second body row: the end of the row above.
        assert_eq!(run(backspace, t, 32), Some((t.to_string(), 27)));
        assert_eq!(run(delete, t, 27), Some((t.to_string(), 27)));
        assert_eq!(run(backspace, "a\n---\nb", 6), Some(("a\nb".into(), 2)));
    }

    #[test]
    fn keeps_html_pairs() {
        let t = "a<b><b>c";
        assert_eq!(drop_empty_marks(t, &parse(t, true), 0), None);
    }

    #[test]
    fn auto_blocks() {
        assert_eq!(auto_block("x\n[] ", 5), Some(Edit::caret("x\n- [ ] ".into(), 8)));
        assert_eq!(auto_block("```", 3), Some(Edit::caret("```\n\n```".into(), 4)));
        assert_eq!(auto_block("```\na\n```", 9), None, "a closing fence stays");
        assert_eq!(auto_block("a [] ", 5), None);
    }

    #[test]
    fn toggles_checks() {
        let t = "- [ ] a\n- [x] b";
        let lines = parse(t, true);
        assert_eq!(toggle_check(t, &lines[0]).unwrap(), "- [x] a\n- [x] b");
        assert_eq!(toggle_check(t, &lines[1]).unwrap(), "- [ ] a\n- [ ] b");
        assert_eq!(toggle_check("- a", &parse("- a", true)[0]), None);
    }

    #[test]
    fn drops_empty_marks_away_from_caret() {
        let t = "a****b";
        let lines = parse(t, true);
        assert_eq!(drop_empty_marks(t, &lines, 3), None);
        assert_eq!(drop_empty_marks(t, &lines, 6), Some(Edit::caret("ab".into(), 2)));
        assert_eq!(drop_empty_marks(t, &lines, 0), Some(Edit::caret("ab".into(), 0)));
    }

    #[test]
    fn finds_words_and_offsets() {
        assert_eq!(word_at("a partial-refund b", 5), 2..16);
        assert_eq!(word_at("a  b", 2), 2..2);
        assert_eq!(char_to_byte("pärn x", 2), 3);
        assert_eq!(byte_to_char("pärn x", 3), 2);
    }
}
