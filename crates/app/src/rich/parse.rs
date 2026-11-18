//! Finds the markdown syntax of each line, so the editor can hide it and style the rest.
//! All ranges are byte offsets into the whole text. It covers the CommonMark and GFM
//! that Linear writes. Syntax it does not know stays plain text.

use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Block {
    Para,
    /// Level 1 to 6.
    Heading(u8),
    Bullet,
    Numbered(u32),
    /// True when checked.
    Check(bool),
    Quote,
    /// A ``` line that opens or closes a code block.
    Fence,
    /// A line inside a code block.
    Code,
    /// `![name](url)` alone on its line.
    Image,
    /// `---`, `***` or `___` alone on its line.
    Rule,
    /// A table row. True for the header row.
    TableRow(bool),
    /// The `| --- | --- |` line under a table header.
    TableSep,
    Blank,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mark {
    Text,
    Bold,
    Italic,
    Strike,
    Code,
    Link { url: Range<usize> },
    /// `@name`.
    Mention,
    /// A table cell border: a pipe and the space around it.
    Pipe,
    /// `<br>`. The editor hides it and keeps a space in its place.
    Break,
    /// Syntax that the editor hides.
    Hidden,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub mark: Mark,
    pub range: Range<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub name: String,
    /// Empty while the upload runs.
    pub url: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColAlign {
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    /// The line without its line break.
    pub range: Range<usize>,
    pub block: Block,
    /// List nesting, 1 for a top level item. A paragraph inside a list item has the
    /// depth of its item. 0 for other blocks.
    pub depth: usize,
    /// Hidden prefix: indent and block marker. Image, rule, fence and table separator
    /// lines hide all of the line.
    pub marker: Range<usize>,
    /// Inline runs after the marker.
    pub spans: Vec<Span>,
    pub image: Option<Image>,
    /// Text of each cell of a table row, without the space around it. Empty for other blocks.
    pub cells: Vec<Range<usize>>,
}

impl Line {
    /// True when the editor hides all of the line.
    pub fn all_hidden(&self) -> bool {
        matches!(self.block, Block::Image | Block::Fence | Block::Rule | Block::TableSep)
    }

    pub fn is_list(&self) -> bool {
        matches!(self.block, Block::Bullet | Block::Numbered(_) | Block::Check(_))
    }

    pub fn is_table(&self) -> bool {
        matches!(self.block, Block::TableRow(_) | Block::TableSep)
    }
}

/// Parses every line of `text`. `headings` false keeps `# x` as a paragraph.
pub fn parse(text: &str, headings: bool) -> Vec<Line> {
    let mut raws = Vec::new();
    let mut at = 0;
    for raw in text.split('\n') {
        raws.push((at..at + raw.len(), raw));
        at += raw.len() + 1;
    }
    let mut out = Vec::with_capacity(raws.len());
    let mut fence = false;
    let mut in_table = false;
    let mut sep_next = false;
    // Open list levels as (indent width, depth).
    let mut lists: Vec<(usize, usize)> = Vec::new();
    for (i, (range, raw)) in raws.iter().enumerate() {
        let range = range.clone();
        let trimmed = raw.trim();
        if trimmed.starts_with("```") {
            fence = !fence;
            in_table = false;
            lists.clear();
            out.push(whole(range, Block::Fence, None));
            continue;
        }
        if fence {
            out.push(plain(range, Block::Code));
            continue;
        }
        if sep_next {
            sep_next = false;
            out.push(whole(range, Block::TableSep, None));
            continue;
        }
        if trimmed.is_empty() {
            in_table = false;
            out.push(plain(range, Block::Blank));
            continue;
        }
        if in_table && has_pipe(raw) {
            out.push(table_row(text, range, false));
            continue;
        }
        in_table = false;
        let next = raws.get(i + 1).map(|(_, r)| *r);
        if has_pipe(raw) && next.and_then(sep_cols) == Some(split_cells(raw).len()) {
            in_table = true;
            sep_next = true;
            lists.clear();
            out.push(table_row(text, range, true));
            continue;
        }
        if let Some(img) = image_line(trimmed) {
            out.push(whole(range, Block::Image, Some(img)));
            continue;
        }
        if is_rule(trimmed) {
            lists.clear();
            out.push(whole(range, Block::Rule, None));
            continue;
        }
        let (block, mut len, _) = block_marker(raw, headings);
        let (indent_bytes, width) = indent(raw);
        let mut depth = 0;
        if matches!(block, Block::Bullet | Block::Numbered(_) | Block::Check(_)) {
            depth = list_depth(&mut lists, width);
        } else if block == Block::Para && width > 0 && !lists.is_empty() {
            // An indented paragraph under a list item belongs to that item.
            depth = lists.iter().rev().find(|(w, _)| *w < width).map_or(1, |(_, d)| *d);
            len = indent_bytes;
        } else {
            lists.clear();
        }
        let marker = range.start..range.start + len;
        let spans = inline(text, marker.end..range.end);
        out.push(Line { range, block, depth, marker, spans, image: None, cells: Vec::new() });
    }
    out
}

fn whole(range: Range<usize>, block: Block, image: Option<Image>) -> Line {
    Line { marker: range.clone(), range, block, depth: 0, spans: Vec::new(), image, cells: Vec::new() }
}

fn plain(range: Range<usize>, block: Block) -> Line {
    let spans = vec![Span { mark: Mark::Text, range: range.clone() }];
    Line { marker: range.start..range.start, range, block, depth: 0, spans, image: None, cells: Vec::new() }
}

/// Leading space of a line: its byte length and its width, with a tab as 4 spaces.
fn indent(line: &str) -> (usize, usize) {
    let bytes = line.bytes().take_while(|b| *b == b' ' || *b == b'\t').count();
    let tabs = line[..bytes].bytes().filter(|b| *b == b'\t').count();
    (bytes, bytes + tabs * 3)
}

/// Depth of a list item with indent `width`. Items nest when they are indented more
/// than the item above, by any amount, so 2, 3 and 4 space nesting all work.
fn list_depth(lists: &mut Vec<(usize, usize)>, width: usize) -> usize {
    while lists.last().is_some_and(|(w, _)| *w > width) {
        lists.pop();
    }
    let depth = match lists.last() {
        Some((w, d)) if *w == width => *d,
        Some((_, d)) => d + 1,
        None => width / 2 + 1,
    };
    if lists.last().is_none_or(|(w, _)| *w != width) {
        lists.push((width, depth));
    }
    depth
}

/// The block kind of a line, the byte length of its marker and its list depth.
pub fn block_marker(line: &str, headings: bool) -> (Block, usize, usize) {
    let hashes = line.bytes().take_while(|b| *b == b'#').count();
    if headings && (1..=6).contains(&hashes) && line[hashes..].starts_with(' ') {
        return (Block::Heading(hashes as u8), hashes + 1, 0);
    }
    if let Some(n) = quote_marker(line) {
        return (Block::Quote, n, 0);
    }
    let (ind, width) = indent(line);
    let depth = width / 2 + 1;
    let l = &line[ind..];
    for (p, done) in [("- [ ] ", false), ("- [x] ", true), ("- [X] ", true), ("* [ ] ", false), ("* [x] ", true)] {
        if l.starts_with(p) || l == p.trim_end() {
            return (Block::Check(done), ind + p.len().min(l.len()), depth);
        }
    }
    if l.starts_with("- ") || l.starts_with("* ") || l.starts_with("+ ") {
        return (Block::Bullet, ind + 2, depth);
    }
    let digits = l.bytes().take_while(u8::is_ascii_digit).count();
    if (1..4).contains(&digits) && (l[digits..].starts_with(". ") || l[digits..].starts_with(") ")) {
        let n = l[..digits].parse().unwrap_or(1);
        return (Block::Numbered(n), ind + digits + 2, depth);
    }
    (Block::Para, 0, 0)
}

/// Byte length of `> `, `>` or nested `> > ` at the line start.
fn quote_marker(line: &str) -> Option<usize> {
    let b = line.as_bytes();
    let mut n = 0;
    while b.get(n) == Some(&b'>') {
        match b.get(n + 1) {
            None => n += 1,
            Some(b' ') => {
                n += 2;
                if b.get(n) == Some(&b' ') && b.get(n + 1) == Some(&b'>') {
                    n += 1;
                }
            }
            Some(b'>') => n += 1,
            Some(_) => break,
        }
    }
    (n > 0).then_some(n)
}

/// `---`, `***` or `___`, three or more, spaces allowed between.
fn is_rule(trimmed: &str) -> bool {
    let Some(c) = trimmed.chars().next().filter(|c| matches!(c, '-' | '*' | '_')) else { return false };
    trimmed.chars().all(|x| x == c || x == ' ') && trimmed.chars().filter(|x| *x == c).count() >= 3
}

/// `![name](url)` or `![name](url "title")` with nothing else on the line.
fn image_line(trimmed: &str) -> Option<Image> {
    let inner = trimmed.strip_prefix("![")?.strip_suffix(')')?;
    let (name, rest) = inner.split_once("](")?;
    let url = match rest.split_once(' ') {
        Some((u, title)) if is_title(title.trim()) => u,
        _ => rest,
    };
    if url.contains(char::is_whitespace) || name.contains(']') {
        return None;
    }
    Some(Image { name: name.to_string(), url: url.to_string() })
}

fn is_title(s: &str) -> bool {
    s.len() >= 2 && ((s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')))
}

/// True when the line has a pipe that is not escaped.
fn has_pipe(line: &str) -> bool {
    !pipes(line).is_empty()
}

fn pipes(line: &str) -> Vec<usize> {
    let b = line.as_bytes();
    let mut out = Vec::new();
    let mut k = 0;
    while k < b.len() {
        match b[k] {
            b'\\' => k += 1,
            b'|' => out.push(k),
            _ => {}
        }
        k += 1;
    }
    out
}

/// The cells of a table line, relative to the line, without the space around them.
/// The pipes at the line start and end are optional.
fn split_cells(line: &str) -> Vec<Range<usize>> {
    let mut segs = Vec::new();
    let mut start = 0;
    for p in pipes(line) {
        segs.push(start..p);
        start = p + 1;
    }
    segs.push(start..line.len());
    if segs.len() > 1 && line[segs[0].clone()].trim().is_empty() {
        segs.remove(0);
    }
    if segs.len() > 1 && segs.last().is_some_and(|s| line[s.clone()].trim().is_empty()) {
        segs.pop();
    }
    segs.into_iter()
        .map(|s| {
            let seg = &line[s.clone()];
            let lead = seg.len() - seg.trim_start().len();
            if lead == seg.len() {
                // An empty cell sits after one space, where the caret goes to type into it.
                let at = s.start + seg.len().min(1);
                return at..at;
            }
            s.start + lead..s.start + seg.trim_end().len()
        })
        .collect()
}

/// The column count of a separator line like `| --- | :-: |`, or None.
fn sep_cols(line: &str) -> Option<usize> {
    if !has_pipe(line) {
        return None;
    }
    let cells = split_cells(line);
    cells.iter().all(|c| is_sep_cell(&line[c.clone()])).then_some(cells.len())
}

fn is_sep_cell(c: &str) -> bool {
    let d = c.strip_prefix(':').unwrap_or(c);
    let d = d.strip_suffix(':').unwrap_or(d);
    !d.is_empty() && d.bytes().all(|b| b == b'-')
}

/// Column alignment from the text of a separator line.
pub fn table_align(sep: &str) -> Vec<ColAlign> {
    split_cells(sep)
        .into_iter()
        .map(|r| {
            let c = &sep[r];
            match (c.starts_with(':'), c.ends_with(':') && c.len() > 1) {
                (true, true) => ColAlign::Center,
                (false, true) => ColAlign::Right,
                _ => ColAlign::Left,
            }
        })
        .collect()
}

fn table_row(text: &str, range: Range<usize>, header: bool) -> Line {
    let cells: Vec<Range<usize>> =
        split_cells(&text[range.clone()]).into_iter().map(|c| range.start + c.start..range.start + c.end).collect();
    let mut spans = Vec::new();
    let mut at = range.start;
    for c in &cells {
        push(&mut spans, Mark::Pipe, at..c.start);
        spans.extend(inline(text, c.clone()));
        at = c.end;
    }
    push(&mut spans, Mark::Pipe, at..range.end);
    Line { marker: range.start..range.start, range, block: Block::TableRow(header), depth: 0, spans, image: None, cells }
}

fn push(out: &mut Vec<Span>, mark: Mark, r: Range<usize>) {
    if !r.is_empty() {
        out.push(Span { mark, range: r });
    }
}

/// Inline runs of `text[range]`: bold, italic, strike, code, links, autolinks, bare
/// urls, mentions, escapes and inline html. Marks nest one level: the inner text takes
/// the outer style, and links and code keep their own.
pub fn inline(text: &str, range: Range<usize>) -> Vec<Span> {
    let mut out = Vec::new();
    inline_into(text, range, &mut out);
    // Joins neighbouring text runs, for example the text after an escape.
    let mut joined: Vec<Span> = Vec::with_capacity(out.len());
    for s in out {
        match joined.last_mut() {
            Some(last) if last.mark == Mark::Text && s.mark == Mark::Text && last.range.end == s.range.start => {
                last.range.end = s.range.end;
            }
            _ => joined.push(s),
        }
    }
    joined
}

enum Tok {
    Runs(Vec<(Mark, Range<usize>)>),
    /// A hidden open mark, a body with its own marks and a hidden close mark.
    Wrap { open: Range<usize>, body: Range<usize>, close: Range<usize>, mark: Mark },
}

impl Tok {
    fn end(&self) -> usize {
        match self {
            Tok::Runs(r) => r.last().map_or(0, |x| x.1.end),
            Tok::Wrap { close, .. } => close.end,
        }
    }
}

fn inline_into(text: &str, range: Range<usize>, out: &mut Vec<Span>) {
    let end = range.end;
    let mut plain = range.start;
    let mut i = range.start;
    while i < end {
        match token(text, i, end) {
            Some(tok) => {
                push(out, Mark::Text, plain..i);
                let stop = tok.end().max(i + 1);
                match tok {
                    Tok::Runs(runs) => runs.into_iter().for_each(|(m, r)| push(out, m, r)),
                    Tok::Wrap { open, body, close, mark } => {
                        push(out, Mark::Hidden, open);
                        let mut inner = Vec::new();
                        inline_into(text, body, &mut inner);
                        for s in inner {
                            push(out, nest(&mark, s.mark), s.range);
                        }
                        push(out, Mark::Hidden, close);
                    }
                }
                i = stop;
                plain = i;
            }
            None => i += text[i..].chars().next().map_or(1, char::len_utf8),
        }
    }
    push(out, Mark::Text, plain..end);
}

/// The mark of an inner run inside `outer`. One style per run, so bold wins over italic.
fn nest(outer: &Mark, inner: Mark) -> Mark {
    match (outer, inner) {
        (_, m @ (Mark::Hidden | Mark::Break | Mark::Code | Mark::Pipe)) => m,
        (Mark::Link { .. }, _) => outer.clone(),
        (_, m @ (Mark::Link { .. } | Mark::Mention)) => m,
        (_, Mark::Text) => outer.clone(),
        (Mark::Bold, _) | (_, Mark::Bold) => Mark::Bold,
        (_, m) => m,
    }
}

/// The token that starts at byte `i`, if any.
fn token(text: &str, i: usize, end: usize) -> Option<Tok> {
    let rest = &text[i..end];
    let prev = text[..i].chars().next_back();
    match rest.as_bytes()[0] {
        b'\\' => match rest[1..].chars().next() {
            Some(c) if c.is_ascii_punctuation() => Some(Tok::Runs(vec![(Mark::Hidden, i..i + 1), (Mark::Text, i + 1..i + 2)])),
            // A backslash at the line end is a hard line break.
            None => Some(Tok::Runs(vec![(Mark::Hidden, i..i + 1)])),
            Some(_) => None,
        },
        b'`' => code_span(rest, i),
        b'*' | b'_' | b'~' => emphasis(rest, i, prev),
        b'!' if rest.starts_with("![") => link(&rest[1..], i + 1).map(|t| match t {
            Tok::Wrap { open, body, close, mark } => Tok::Wrap { open: open.start - 1..open.end, body, close, mark },
            t => t,
        }),
        b'[' => link(rest, i),
        b'<' => angle(rest, i),
        b'@' => mention(rest, i, prev),
        b'h' | b'H' | b'w' | b'W' => bare_url(rest, i, prev),
        _ => None,
    }
}

/// `` `code` `` or ``` ``code`` ```. A lone pair of backticks is empty code, which the
/// code shortcut leaves while the user types into it.
fn code_span(rest: &str, at: usize) -> Option<Tok> {
    let n = rest.bytes().take_while(|b| *b == b'`').count();
    let fence = &rest[..n];
    let mut k = n;
    while let Some(j) = rest[k..].find(fence) {
        let p = k + j;
        let run = rest[p..].bytes().take_while(|b| *b == b'`').count();
        if run == n {
            let body = at + n..at + p;
            return Some(Tok::Runs(vec![(Mark::Hidden, at..at + n), (Mark::Code, body), (Mark::Hidden, at + p..at + p + n)]));
        }
        k = p + run;
    }
    if n == 2 {
        return Some(Tok::Runs(vec![(Mark::Hidden, at..at + 1), (Mark::Hidden, at + 1..at + 2)]));
    }
    None
}

/// `**b**`, `__b__`, `*i*`, `_i_`, `***b***` and `~~s~~`.
fn emphasis(rest: &str, at: usize, prev: Option<char>) -> Option<Tok> {
    let ch = rest.as_bytes()[0];
    let run = rest.bytes().take_while(|b| *b == ch).count();
    if ch == b'~' {
        return if run >= 2 { delim(rest, at, 2, Mark::Strike, false) } else { None };
    }
    let word = ch == b'_';
    // `_` inside a word, as in snake_case, is text.
    if word && prev.is_some_and(char::is_alphanumeric) {
        return None;
    }
    let tries: &[(usize, Mark)] = match run {
        1 => &[(1, Mark::Italic)],
        2 => &[(2, Mark::Bold), (1, Mark::Italic)],
        _ => &[(3, Mark::Bold), (2, Mark::Bold), (1, Mark::Italic)],
    };
    tries.iter().find_map(|(n, m)| delim(rest, at, *n, m.clone(), word))
}

/// A pair of `n` delimiter chars at the start of `rest`. The body may be empty for
/// double marks, so a fresh `****` from the bold shortcut stays hidden while the user
/// types into it.
fn delim(rest: &str, at: usize, n: usize, mark: Mark, word: bool) -> Option<Tok> {
    let d = &rest[..n];
    let body = &rest[n..];
    if n >= 2 && body.starts_with(d) {
        let e = at + n;
        return Some(Tok::Wrap { open: at..e, body: e..e, close: e..e + n, mark });
    }
    let ch = d.as_bytes()[0];
    if body.is_empty() || body.starts_with(char::is_whitespace) || body.as_bytes()[0] == ch {
        return None;
    }
    let j = find_close(body, ch, n, word)?;
    let b = at + n..at + n + j;
    Some(Tok::Wrap { open: at..at + n, body: b.clone(), close: b.end..b.end + n, mark })
}

/// Offset in `body` of the closing run of `n` times `ch`. Skips escapes, code and runs
/// of another length, so `*a **b** c*` closes at the last star.
fn find_close(body: &str, ch: u8, n: usize, word: bool) -> Option<usize> {
    let b = body.as_bytes();
    let mut k = 0;
    while k < b.len() {
        match b[k] {
            b'\\' => k += 2,
            b'`' => {
                let run = b[k..].iter().take_while(|x| **x == b'`').count();
                let fence = &body[k..k + run];
                k = body[k + run..].find(fence).map_or(k + run, |j| k + run + j + run);
            }
            c if c == ch => {
                let run = b[k..].iter().take_while(|x| **x == ch).count();
                let at = match (n, run) {
                    (1, 1) | (2, 2) => Some(k),
                    (3, r) if r >= 3 => Some(k),
                    (1, 3) => Some(k + 2),
                    (2, 3) => Some(k + 1),
                    _ => None,
                };
                if let Some(p) = at {
                    let before = body[..p].chars().next_back();
                    let after = body[p + n..].chars().next();
                    let ok = p > 0
                        && before.is_some_and(|c| !c.is_whitespace())
                        && !(word && after.is_some_and(char::is_alphanumeric));
                    if ok {
                        return Some(p);
                    }
                }
                k += run;
            }
            _ => k += 1,
        }
    }
    None
}

/// `[text](url)` or `[text](url "title")` at the start of `s`, which starts at byte `at`.
fn link(s: &str, at: usize) -> Option<Tok> {
    let b = s.as_bytes();
    let mut depth = 0;
    let mut close = None;
    let mut k = 0;
    while k < b.len() {
        match b[k] {
            b'\\' => k += 1,
            b'[' => depth += 1,
            b']' => {
                depth -= 1;
                if depth == 0 {
                    close = Some(k);
                    break;
                }
            }
            _ => {}
        }
        k += 1;
    }
    let close = close?;
    if close <= 1 || !s[close + 1..].starts_with('(') {
        return None;
    }
    let d0 = close + 2;
    let d = &s[d0..];
    let (url, after) = if let Some(inner) = d.strip_prefix('<') {
        let e = inner.find('>')?;
        (d0 + 1..d0 + 1 + e, d0 + e + 2)
    } else {
        let mut parens = 0;
        let mut e = d.len();
        for (j, c) in d.char_indices() {
            match c {
                '(' => parens += 1,
                ')' if parens == 0 => {
                    e = j;
                    break;
                }
                ')' => parens -= 1,
                c if c.is_whitespace() => {
                    e = j;
                    break;
                }
                _ => {}
            }
        }
        (d0..d0 + e, d0 + e)
    };
    let tail = &s[after..];
    let title = tail.trim_start();
    let end = if let Some(r) = title.strip_prefix(')') {
        s.len() - r.len()
    } else {
        let q = title.chars().next().filter(|c| matches!(c, '"' | '\''))?;
        let e = title[1..].find(q)?;
        let r = title[1 + e + 1..].trim_start().strip_prefix(')')?;
        s.len() - r.len()
    };
    if url.is_empty() && !s[close + 1..].starts_with("()") {
        return None;
    }
    let url = at + url.start..at + url.end;
    Some(Tok::Wrap { open: at..at + 1, body: at + 1..at + close, close: at + close..at + end, mark: Mark::Link { url } })
}

/// `<https://x>` and `<a@b.c>` autolinks, `<br>`, other html tags and html comments.
fn angle(rest: &str, at: usize) -> Option<Tok> {
    if let Some(body) = rest.strip_prefix("<!--") {
        let e = body.find("-->")?;
        return Some(Tok::Runs(vec![(Mark::Hidden, at..at + 4 + e + 3)]));
    }
    let e = rest[1..].find('>')?;
    let inner = &rest[1..1 + e];
    let whole = at..at + e + 2;
    if is_autolink(inner) {
        let url = at + 1..at + 1 + e;
        return Some(Tok::Runs(vec![(Mark::Hidden, at..at + 1), (Mark::Link { url: url.clone() }, url), (Mark::Hidden, whole.end - 1..whole.end)]));
    }
    let name = tag_name(inner)?;
    let mark = if name.eq_ignore_ascii_case("br") { Mark::Break } else { Mark::Hidden };
    Some(Tok::Runs(vec![(mark, whole)]))
}

fn is_autolink(s: &str) -> bool {
    if s.is_empty() || s.contains(|c: char| c.is_whitespace() || c == '<') {
        return false;
    }
    if let Some((scheme, rest)) = s.split_once(':') {
        let ok = (2..=32).contains(&scheme.len())
            && scheme.starts_with(|c: char| c.is_ascii_alphabetic())
            && scheme.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'));
        return ok && !rest.is_empty();
    }
    let Some((user, host)) = s.split_once('@') else { return false };
    !user.is_empty() && host.contains('.') && !host.starts_with('.') && !host.ends_with('.')
}

/// The name of an html tag like `b`, `/b`, `br/` or `a href="x"`.
fn tag_name(inner: &str) -> Option<&str> {
    let s = inner.strip_prefix('/').unwrap_or(inner);
    let n = s.bytes().take_while(|b| b.is_ascii_alphanumeric() || *b == b'-').count();
    if n == 0 || !s.starts_with(|c: char| c.is_ascii_alphabetic()) {
        return None;
    }
    let after = &s[n..];
    (after.is_empty() || after == "/" || after.starts_with(char::is_whitespace)).then_some(&s[..n])
}

/// `@name` after a space or at the start.
fn mention(rest: &str, at: usize, prev: Option<char>) -> Option<Tok> {
    if prev.is_some_and(|c| !c.is_whitespace() && c != '(') {
        return None;
    }
    let name = &rest[1..];
    if !name.starts_with(|c: char| c.is_alphanumeric()) {
        return None;
    }
    let n: usize = name.chars().take_while(|c| c.is_alphanumeric() || matches!(c, '.' | '_' | '-')).map(char::len_utf8).sum();
    let n = name[..n].trim_end_matches(['.', '_', '-']).len();
    Some(Tok::Runs(vec![(Mark::Mention, at..at + 1 + n)]))
}

/// A bare `https://`, `http://` or `www.` url. Trailing punctuation stays text.
fn bare_url(rest: &str, at: usize, prev: Option<char>) -> Option<Tok> {
    if prev.is_some_and(|c| !c.is_whitespace() && !matches!(c, '(' | '*' | '_' | '~' | '"' | '\'')) {
        return None;
    }
    let lower = rest.get(..8).unwrap_or(rest).to_ascii_lowercase();
    let scheme = ["https://", "http://", "www."].into_iter().find(|p| lower.starts_with(p))?;
    let mut e = rest.find(|c: char| c.is_whitespace() || c == '<').unwrap_or(rest.len());
    loop {
        let u = &rest[..e];
        if u.ends_with(['?', '!', '.', ',', ':', ';', '*', '_', '~', '\'', '"']) {
            e -= 1;
        } else if u.ends_with(')') && u.matches(')').count() > u.matches('(').count() {
            e -= 1;
        } else {
            break;
        }
    }
    if e <= scheme.len() {
        return None;
    }
    let r = at..at + e;
    Some(Tok::Runs(vec![(Mark::Link { url: r.clone() }, r)]))
}

/// The address to open for a link url: `www.` urls get https and emails get mailto.
pub fn link_target(url: &str) -> String {
    if url.get(..4).is_some_and(|p| p.eq_ignore_ascii_case("www.")) {
        return format!("https://{url}");
    }
    if !url.contains(':') && url.contains('@') {
        return format!("mailto:{url}");
    }
    url.to_string()
}

/// How the caret treats a hidden range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hide {
    /// A block marker. The caret stays after it.
    Marker,
    /// A whole hidden line. The caret skips the line.
    Line,
    /// An inline mark. The caret may sit at either edge.
    Inline,
}

/// Every hidden range of the text, in order.
pub fn hidden(lines: &[Line]) -> Vec<(Range<usize>, Hide)> {
    let mut out = Vec::new();
    for l in lines {
        if l.all_hidden() {
            out.push((l.range.clone(), Hide::Line));
            continue;
        }
        if !l.marker.is_empty() {
            out.push((l.marker.clone(), Hide::Marker));
        }
        for s in &l.spans {
            if matches!(s.mark, Mark::Hidden | Mark::Pipe | Mark::Break) {
                out.push((s.range.clone(), Hide::Inline));
            }
        }
    }
    out
}

/// The line that holds byte `at`. A position at a line end belongs to that line.
pub fn line_at(lines: &[Line], at: usize) -> Option<usize> {
    lines.iter().position(|l| l.range.start <= at && at <= l.range.end)
}

/// Line index ranges of the tables: a header row, its separator and the body rows.
pub fn tables(lines: &[Line]) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if lines[i].block == Block::TableRow(true) {
            let start = i;
            i += 1;
            while i < lines.len() && matches!(lines[i].block, Block::TableSep | Block::TableRow(false)) {
                i += 1;
            }
            out.push(start..i);
        } else {
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marks(text: &str) -> Vec<(Mark, &str)> {
        inline(text, 0..text.len()).into_iter().map(|s| (s.mark, &text[s.range])).collect()
    }

    #[test]
    fn splits_inline_marks() {
        let t = "a **b** `c` [d](e) ~~f~~ *g*";
        let m = marks(t);
        let text: Vec<&str> = m.iter().map(|x| x.1).collect();
        assert_eq!(
            text,
            vec!["a ", "**", "b", "**", " ", "`", "c", "`", " ", "[", "d", "](e)", " ", "~~", "f", "~~", " ", "*", "g", "*"]
        );
        assert_eq!(m[2].0, Mark::Bold);
        assert_eq!(m[6].0, Mark::Code);
        assert_eq!(m[10].0, Mark::Link { url: 16..17 });
        assert_eq!(m[14].0, Mark::Strike);
        assert_eq!(m[18].0, Mark::Italic);
    }

    #[test]
    fn unclosed_marks_stay_text() {
        assert_eq!(marks("a ** b ` c [d]"), vec![(Mark::Text, "a ** b ` c [d]")]);
        assert_eq!(marks("2 * 3 * 4"), vec![(Mark::Text, "2 * 3 * 4")]);
        assert_eq!(marks("[x] (y)"), vec![(Mark::Text, "[x] (y)")]);
    }

    #[test]
    fn empty_pairs_are_hidden() {
        assert_eq!(marks("a****b"), vec![(Mark::Text, "a"), (Mark::Hidden, "**"), (Mark::Hidden, "**"), (Mark::Text, "b")]);
        assert_eq!(marks("``"), vec![(Mark::Hidden, "`"), (Mark::Hidden, "`")]);
    }

    #[test]
    fn escapes_hide_the_backslash() {
        assert_eq!(marks(r"a \*b\* c"), vec![(Mark::Text, "a "), (Mark::Hidden, "\\"), (Mark::Text, "*b"), (Mark::Hidden, "\\"), (Mark::Text, "* c")]);
    }

    #[test]
    fn inline_image_shows_its_name() {
        let m = marks("see ![shot](u) now");
        assert_eq!(m[1], (Mark::Hidden, "!["));
        assert_eq!(m[2].1, "shot");
        assert_eq!(m[3], (Mark::Hidden, "](u)"));
    }

    #[test]
    fn finds_blocks() {
        let t = "# A\n## B\n### C\n- d\n* e\n1. f\n12) g\n- [ ] h\n- [x] i\n> j\nk\n\n  - l";
        let lines = parse(t, true);
        let blocks: Vec<Block> = lines.iter().map(|l| l.block).collect();
        assert_eq!(
            blocks,
            vec![
                Block::Heading(1),
                Block::Heading(2),
                Block::Heading(3),
                Block::Bullet,
                Block::Bullet,
                Block::Numbered(1),
                Block::Numbered(12),
                Block::Check(false),
                Block::Check(true),
                Block::Quote,
                Block::Para,
                Block::Blank,
                Block::Bullet,
            ]
        );
        let marker = |i: usize| &t[lines[i].marker.clone()];
        assert_eq!(marker(1), "## ");
        assert_eq!(marker(7), "- [ ] ");
        assert_eq!(marker(6), "12) ");
        assert_eq!(marker(12), "  - ");
        assert_eq!(lines[12].depth, 2);
        assert_eq!(lines[3].depth, 1);
        assert_eq!(&t[lines[8].spans[0].range.clone()], "i");
    }

    #[test]
    fn headings_can_be_off() {
        assert_eq!(parse("# x", false)[0].block, Block::Para);
        assert_eq!(parse("#x", true)[0].block, Block::Para);
    }

    #[test]
    fn empty_markers_still_count() {
        assert_eq!(parse("- ", true)[0].block, Block::Bullet);
        assert_eq!(parse("- [ ]", true)[0].block, Block::Check(false));
        assert_eq!(parse(">", true)[0].block, Block::Quote);
    }

    #[test]
    fn finds_code_blocks_and_images() {
        let t = "a\n```rust\nlet **x** = 1;\n```\n![shot.png](https://x/y.png)\n![Uploading p.png...]()";
        let lines = parse(t, true);
        let blocks: Vec<Block> = lines.iter().map(|l| l.block).collect();
        assert_eq!(blocks, vec![Block::Para, Block::Fence, Block::Code, Block::Fence, Block::Image, Block::Image]);
        assert_eq!(lines[2].spans.len(), 1, "no marks inside code");
        assert_eq!(lines[4].image, Some(Image { name: "shot.png".into(), url: "https://x/y.png".into() }));
        assert_eq!(lines[5].image.as_ref().map(|i| i.url.as_str()), Some(""));
        assert!(lines[4].all_hidden());
    }

    #[test]
    fn unclosed_fence_runs_to_the_end() {
        let lines = parse("```\na\nb", true);
        assert_eq!(lines[2].block, Block::Code);
    }

    #[test]
    fn lists_hidden_ranges() {
        let t = "## H\n- **b**\n![a](u)";
        let h = hidden(&parse(t, true));
        let got: Vec<(&str, Hide)> = h.iter().map(|(r, k)| (&t[r.clone()], *k)).collect();
        assert_eq!(
            got,
            vec![("## ", Hide::Marker), ("- ", Hide::Marker), ("**", Hide::Inline), ("**", Hide::Inline), ("![a](u)", Hide::Line)]
        );
    }

    #[test]
    fn finds_line_at() {
        let lines = parse("ab\ncd", true);
        assert_eq!(line_at(&lines, 2), Some(0));
        assert_eq!(line_at(&lines, 3), Some(1));
        assert_eq!(line_at(&lines, 5), Some(1));
    }

    /// The text a reader sees: every run that is not hidden, line by line.
    fn visible(text: &str) -> Vec<String> {
        parse(text, true)
            .iter()
            .map(|l| {
                if let Some(img) = &l.image {
                    return img.name.clone();
                }
                l.spans
                    .iter()
                    .filter(|s| !matches!(s.mark, Mark::Hidden | Mark::Pipe | Mark::Break) && !l.all_hidden())
                    .map(|s| &text[s.range.clone()])
                    .collect::<Vec<_>>()
                    .join("|")
            })
            .collect()
    }

    #[test]
    fn finds_tables() {
        let t = "| a | b |\n| -- | :-: |\n| 1 | **2** |\nafter";
        let lines = parse(t, true);
        let blocks: Vec<Block> = lines.iter().map(|l| l.block).collect();
        assert_eq!(blocks, vec![Block::TableRow(true), Block::TableSep, Block::TableRow(false), Block::Para]);
        let cells: Vec<&str> = lines[0].cells.iter().map(|c| &t[c.clone()]).collect();
        assert_eq!(cells, vec!["a", "b"]);
        let m: Vec<(Mark, &str)> = lines[0].spans.iter().map(|s| (s.mark.clone(), &t[s.range.clone()])).collect();
        assert_eq!(m, vec![(Mark::Pipe, "| "), (Mark::Text, "a"), (Mark::Pipe, " | "), (Mark::Text, "b"), (Mark::Pipe, " |")]);
        assert!(lines[1].all_hidden());
        assert!(lines[2].spans.iter().any(|s| s.mark == Mark::Bold && &t[s.range.clone()] == "2"));
        assert_eq!(table_align(&t[lines[1].range.clone()]), vec![ColAlign::Left, ColAlign::Center]);
        assert_eq!(tables(&lines), vec![0..3]);
    }

    #[test]
    fn tables_need_a_matching_separator() {
        assert_eq!(parse("a | b", true)[0].block, Block::Para);
        assert_eq!(parse("| a | b |\n| -- |", true)[0].block, Block::Para);
        let t = "a | b\n--- | ---\nc | d";
        let lines = parse(t, true);
        assert_eq!(lines[2].block, Block::TableRow(false));
        let cells: Vec<&str> = lines[2].cells.iter().map(|c| &t[c.clone()]).collect();
        assert_eq!(cells, vec!["c", "d"]);
        // A blank line ends the table.
        assert_eq!(parse("| a |\n| - |\n\n| b |", true)[3].block, Block::Para);
    }

    #[test]
    fn empty_cells_sit_after_one_space() {
        let t = "| a |  |\n| - | - |\n|  |  |";
        let lines = parse(t, true);
        assert_eq!(lines[0].cells[1], 6..6);
        let row = lines[2].range.start;
        assert_eq!(lines[2].cells, vec![row + 2..row + 2, row + 5..row + 5]);
    }

    #[test]
    fn escaped_pipes_stay_in_the_cell() {
        let t = r"| a \| b | c |
| - | - |";
        let lines = parse(t, true);
        assert_eq!(&t[lines[0].cells[0].clone()], r"a \| b");
        let cell = lines[0].cells[0].clone();
        let shown: String = lines[0]
            .spans
            .iter()
            .filter(|s| s.range.start >= cell.start && s.range.end <= cell.end && s.mark != Mark::Hidden)
            .map(|s| &t[s.range.clone()])
            .collect();
        assert_eq!(shown, "a | b");
    }

    #[test]
    fn autolinks_hide_the_brackets() {
        let m = marks("see <https://example.com/x> and <me@x.io>");
        assert_eq!(m[1], (Mark::Hidden, "<"));
        assert_eq!(m[2], (Mark::Link { url: 5..26 }, "https://example.com/x"));
        assert_eq!(m[3], (Mark::Hidden, ">"));
        assert_eq!(m[6].1, "me@x.io");
        assert!(matches!(m[6].0, Mark::Link { .. }));
        assert_eq!(link_target("me@x.io"), "mailto:me@x.io");
        assert_eq!(link_target("www.x.io"), "https://www.x.io");
        assert_eq!(link_target("https://x.io"), "https://x.io");
    }

    #[test]
    fn finds_bare_urls() {
        let m = marks("see https://x.com/a_(b). Or (www.y.io)");
        assert_eq!(m[1], (Mark::Link { url: 4..23 }, "https://x.com/a_(b)"));
        assert_eq!(m[2].1, ". Or (");
        assert_eq!(m[3].1, "www.y.io");
        assert_eq!(marks("xhttps://a.b"), vec![(Mark::Text, "xhttps://a.b")]);
        assert_eq!(marks("https://"), vec![(Mark::Text, "https://")]);
    }

    #[test]
    fn escapes_and_hard_breaks() {
        assert_eq!(marks(r"\# \~x"), vec![(Mark::Hidden, "\\"), (Mark::Text, "# "), (Mark::Hidden, "\\"), (Mark::Text, "~x")]);
        assert_eq!(marks(r"line\"), vec![(Mark::Text, "line"), (Mark::Hidden, "\\")]);
        assert_eq!(marks(r"a\b"), vec![(Mark::Text, r"a\b")]);
        assert_eq!(visible("a  \nb"), vec!["a  ", "b"]);
    }

    #[test]
    fn strike_and_italics() {
        assert_eq!(marks("~~x y~~")[1], (Mark::Strike, "x y"));
        assert_eq!(marks("~x~"), vec![(Mark::Text, "~x~")]);
        assert_eq!(marks("_x_")[1], (Mark::Italic, "x"));
        assert_eq!(marks("a *x y* b")[2], (Mark::Italic, "x y"));
        assert_eq!(marks("snake_case_name"), vec![(Mark::Text, "snake_case_name")]);
        assert_eq!(marks("__b__")[1], (Mark::Bold, "b"));
        assert_eq!(marks("***b***")[1], (Mark::Bold, "b"));
        assert_eq!(marks("a ** b ** c"), vec![(Mark::Text, "a ** b ** c")]);
    }

    #[test]
    fn marks_nest() {
        let m = marks("*a **b** c*");
        assert_eq!(m.iter().filter(|x| x.0 != Mark::Hidden).map(|x| x.1).collect::<String>(), "a b c");
        assert!(m.contains(&(Mark::Bold, "b")));
        assert!(m.contains(&(Mark::Italic, "a ")));
        let m = marks("**see [docs](u)**");
        assert!(m.contains(&(Mark::Link { url: 13..14 }, "docs")));
        assert!(m.contains(&(Mark::Bold, "see ")));
        let m = marks("[**b** c](u)");
        assert_eq!(m.iter().filter(|x| x.0 != Mark::Hidden).count(), 2);
        assert!(m.iter().all(|x| x.0 == Mark::Hidden || matches!(x.0, Mark::Link { .. })));
        assert!(marks("**`x`**").contains(&(Mark::Code, "x")));
    }

    #[test]
    fn links_with_titles_and_brackets() {
        let m = marks(r#"[a](https://x "t") b"#);
        assert_eq!(m[1], (Mark::Link { url: 4..13 }, "a"));
        assert_eq!(m[2], (Mark::Hidden, r#"](https://x "t")"#));
        let m = marks("[a [b] c](<u v>)");
        assert_eq!(m[1].1, "a [b] c");
        assert_eq!(m[1].0, Mark::Link { url: 11..14 });
        assert_eq!(marks("[a](b c)"), vec![(Mark::Text, "[a](b c)")]);
        assert_eq!(parse(r#"![s.png](https://x/s.png "Shot")"#, true)[0].image.as_ref().map(|i| i.url.as_str()), Some("https://x/s.png"));
    }

    #[test]
    fn html_tags_are_hidden() {
        assert_eq!(marks("a<br>b"), vec![(Mark::Text, "a"), (Mark::Break, "<br>"), (Mark::Text, "b")]);
        assert_eq!(marks("a<BR />b")[1], (Mark::Break, "<BR />"));
        assert_eq!(visible(r#"<u>x</u> <a href="h">y</a> <!-- c -->z"#), vec!["x| |y| |z"]);
        assert_eq!(marks("1 < 2 > 0"), vec![(Mark::Text, "1 < 2 > 0")]);
    }

    #[test]
    fn finds_mentions() {
        assert_eq!(marks("@kaarel.o, hi")[0], (Mark::Mention, "@kaarel.o"));
        assert_eq!(marks("(@mt)")[1], (Mark::Mention, "@mt"));
        assert_eq!(marks("a@b.c"), vec![(Mark::Text, "a@b.c")]);
        assert_eq!(marks("@ x"), vec![(Mark::Text, "@ x")]);
    }

    #[test]
    fn nested_lists_follow_the_indent() {
        let t = "- a\n    - b\n        - c\n    - d\n- e\n1. f\n   - g\n     more";
        let lines = parse(t, true);
        let depth: Vec<usize> = lines.iter().map(|l| l.depth).collect();
        assert_eq!(depth, vec![1, 2, 3, 2, 1, 1, 2, 2]);
        assert_eq!(lines[7].block, Block::Para);
        assert_eq!(&t[lines[7].marker.clone()], "     ");
        assert_eq!(parse("\t- x", true)[0].depth, 3);
    }

    #[test]
    fn rules_and_nested_quotes() {
        let lines = parse("a\n---\n* * *\n> > q", true);
        assert_eq!(lines[1].block, Block::Rule);
        assert_eq!(lines[2].block, Block::Rule);
        assert!(lines[1].all_hidden());
        assert_eq!(lines[3].block, Block::Quote);
        assert_eq!(lines[3].marker, 12..16);
        assert_eq!(parse(">5", true)[0].block, Block::Para);
    }

    /// Shaped like the user's real comments: a table with an escaped first header cell,
    /// autolinks, bold, a link, an image and a heading.
    #[test]
    fn user_comment_shows_no_syntax() {
        let t = "## Test results\n\n\
                 | \\# | Case | Result |\n\
                 | -- | -- | -- |\n\
                 | 1 | **Partial refund** | <https://linear.app/acme/issue/WEB-214> |\n\
                 | 2 | [Full refund](https://docs.example.com/refunds) | \\~ ok |\n\n\
                 Checked on <https://sandbox.example.com/orders/4471>, see **notes**.\n\n\
                 ![shot.png](https://uploads.linear.app/a/shot.png)";
        let lines = parse(t, true);
        assert_eq!(lines[0].block, Block::Heading(2));
        assert_eq!(lines[2].block, Block::TableRow(true));
        assert_eq!(&t[lines[2].cells[0].clone()], "\\#");
        assert_eq!(lines[3].block, Block::TableSep);
        assert_eq!(lines[9].block, Block::Image);
        let seen = visible(t);
        assert_eq!(seen[2], "#|Case|Result");
        assert_eq!(seen[4], "1|Partial refund|https://linear.app/acme/issue/WEB-214");
        assert_eq!(seen[5], "2|Full refund|~ ok");
        assert_eq!(seen[7], "Checked on |https://sandbox.example.com/orders/4471|, see |notes|.");
        for (i, s) in seen.iter().enumerate() {
            for raw in ["**", "](", "<", ">", "\\", "| ", "--", "##"] {
                assert!(!s.contains(raw), "line {i} shows {raw:?}: {s:?}");
            }
        }
        let links: Vec<&str> = lines
            .iter()
            .flat_map(|l| &l.spans)
            .filter_map(|s| match &s.mark {
                Mark::Link { url } => Some(&t[url.clone()]),
                _ => None,
            })
            .collect();
        assert_eq!(
            links,
            vec![
                "https://linear.app/acme/issue/WEB-214",
                "https://docs.example.com/refunds",
                "https://sandbox.example.com/orders/4471"
            ]
        );
    }
}
