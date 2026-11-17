//! Finds the markdown syntax of each line, so the editor can hide it and style the rest.
//! All ranges are byte offsets into the whole text.

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    /// The line without its line break.
    pub range: Range<usize>,
    pub block: Block,
    /// List nesting, 1 for a top level item. 0 for other blocks.
    pub depth: usize,
    /// Hidden prefix: indent and block marker. Image and fence lines hide all of the line.
    pub marker: Range<usize>,
    /// Inline runs after the marker.
    pub spans: Vec<Span>,
    pub image: Option<Image>,
}

impl Line {
    /// True when the editor hides all of the line (image and fence lines).
    pub fn all_hidden(&self) -> bool {
        matches!(self.block, Block::Image | Block::Fence)
    }

    pub fn is_list(&self) -> bool {
        matches!(self.block, Block::Bullet | Block::Numbered(_) | Block::Check(_))
    }
}

/// Parses every line of `text`. `headings` false keeps `# x` as a paragraph.
pub fn parse(text: &str, headings: bool) -> Vec<Line> {
    let mut out = Vec::new();
    let mut at = 0;
    let mut fence = false;
    for raw in text.split('\n') {
        let range = at..at + raw.len();
        at += raw.len() + 1;
        let trimmed = raw.trim();
        if trimmed.starts_with("```") {
            fence = !fence;
            out.push(whole(range, Block::Fence, None));
            continue;
        }
        if fence {
            let spans = vec![Span { mark: Mark::Text, range: range.clone() }];
            out.push(Line { marker: range.start..range.start, range, block: Block::Code, depth: 0, spans, image: None });
            continue;
        }
        if trimmed.is_empty() {
            let spans = vec![Span { mark: Mark::Text, range: range.clone() }];
            out.push(Line { marker: range.start..range.start, range, block: Block::Blank, depth: 0, spans, image: None });
            continue;
        }
        if let Some(img) = image_line(trimmed) {
            out.push(whole(range, Block::Image, Some(img)));
            continue;
        }
        let (block, len, depth) = block_marker(raw, headings);
        let marker = range.start..range.start + len;
        let spans = inline(text, marker.end..range.end);
        out.push(Line { range, block, depth, marker, spans, image: None });
    }
    out
}

fn whole(range: Range<usize>, block: Block, image: Option<Image>) -> Line {
    Line { marker: range.clone(), range, block, depth: 0, spans: Vec::new(), image }
}

/// The block kind of a line, the byte length of its marker and its list depth.
pub fn block_marker(line: &str, headings: bool) -> (Block, usize, usize) {
    let hashes = line.bytes().take_while(|b| *b == b'#').count();
    if headings && (1..=6).contains(&hashes) && line[hashes..].starts_with(' ') {
        return (Block::Heading(hashes as u8), hashes + 1, 0);
    }
    if line.starts_with("> ") {
        return (Block::Quote, 2, 0);
    }
    if line == ">" {
        return (Block::Quote, 1, 0);
    }
    let indent = line.len() - line.trim_start_matches(' ').len();
    let depth = indent / 2 + 1;
    let l = &line[indent..];
    for (p, done) in [("- [ ] ", false), ("- [x] ", true), ("- [X] ", true), ("* [ ] ", false), ("* [x] ", true)] {
        if l.starts_with(p) || l == p.trim_end() {
            return (Block::Check(done), indent + p.len().min(l.len()), depth);
        }
    }
    if l.starts_with("- ") || l.starts_with("* ") || l.starts_with("+ ") {
        return (Block::Bullet, indent + 2, depth);
    }
    let digits = l.bytes().take_while(u8::is_ascii_digit).count();
    if (1..4).contains(&digits) && (l[digits..].starts_with(". ") || l[digits..].starts_with(") ")) {
        let n = l[..digits].parse().unwrap_or(1);
        return (Block::Numbered(n), indent + digits + 2, depth);
    }
    (Block::Para, 0, 0)
}

/// `![name](url)` with nothing else on the line.
fn image_line(trimmed: &str) -> Option<Image> {
    let inner = trimmed.strip_prefix("![")?.strip_suffix(')')?;
    let (name, url) = inner.split_once("](")?;
    if url.contains(char::is_whitespace) || name.contains(']') {
        return None;
    }
    Some(Image { name: name.to_string(), url: url.to_string() })
}

/// Inline runs of `text[range]`: bold, italic, strike, code, links and escapes.
/// Unclosed marks stay text. Marks do not nest.
pub fn inline(text: &str, range: Range<usize>) -> Vec<Span> {
    let mut out: Vec<Span> = Vec::new();
    let end = range.end;
    let mut plain = range.start;
    let mut i = range.start;
    let push = |out: &mut Vec<Span>, mark: Mark, r: Range<usize>| {
        if !r.is_empty() {
            out.push(Span { mark, range: r });
        }
    };
    while i < end {
        let rest = &text[i..end];
        let found: Option<Vec<(Mark, Range<usize>)>> = if rest.starts_with('\\')
            && rest[1..].starts_with(|c: char| c.is_ascii_punctuation())
        {
            Some(vec![(Mark::Hidden, i..i + 1), (Mark::Text, i + 1..i + 2)])
        } else if rest.starts_with("**") {
            pair(rest, i, "**", Mark::Bold)
        } else if rest.starts_with("~~") {
            pair(rest, i, "~~", Mark::Strike)
        } else if rest.starts_with('`') {
            pair(rest, i, "`", Mark::Code)
        } else if rest.starts_with('*') {
            italic(rest, i)
        } else if rest.starts_with("![") {
            link_at(&rest[1..], i + 1).map(|mut v| {
                v[0].1.start = i;
                v
            })
        } else if rest.starts_with('[') {
            link_at(rest, i)
        } else {
            None
        };
        match found {
            Some(runs) => {
                push(&mut out, Mark::Text, plain..i);
                let stop = runs.last().map_or(i, |r| r.1.end);
                for (m, r) in runs {
                    push(&mut out, m, r);
                }
                i = stop;
                plain = i;
            }
            None => i += rest.chars().next().map_or(1, char::len_utf8),
        }
    }
    push(&mut out, Mark::Text, plain..end);
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

/// `mark body mark` at the start of `rest`. The body may be empty, so a fresh `****`
/// from the bold shortcut stays hidden while the user types into it.
fn pair(rest: &str, at: usize, mark: &str, kind: Mark) -> Option<Vec<(Mark, Range<usize>)>> {
    let n = mark.len();
    let j = rest[n..].find(mark)?;
    let body = at + n..at + n + j;
    Some(vec![(Mark::Hidden, at..at + n), (kind, body.clone()), (Mark::Hidden, body.end..body.end + n)])
}

/// `*x*` where the body does not start or end with a space.
fn italic(rest: &str, at: usize) -> Option<Vec<(Mark, Range<usize>)>> {
    let body = &rest[1..];
    if body.starts_with([' ', '*']) || body.is_empty() {
        return None;
    }
    let j = body.find('*')?;
    if j == 0 || body[..j].ends_with(' ') || body[j + 1..].starts_with('*') {
        return None;
    }
    let b = at + 1..at + 1 + j;
    Some(vec![(Mark::Hidden, at..at + 1), (Mark::Italic, b.clone()), (Mark::Hidden, b.end..b.end + 1)])
}

/// `[text](url)` at the start of `s`, which starts at byte `at`.
fn link_at(s: &str, at: usize) -> Option<Vec<(Mark, Range<usize>)>> {
    let close = s[1..].find(']')?;
    if close == 0 {
        return None;
    }
    let after = &s[1 + close + 1..];
    if !after.starts_with('(') {
        return None;
    }
    let end = after[1..].find(')')?;
    if after[1..1 + end].contains(char::is_whitespace) {
        return None;
    }
    let text = at + 1..at + 1 + close;
    let url = text.end + 2..text.end + 2 + end;
    Some(vec![
        (Mark::Hidden, at..text.start),
        (Mark::Link { url: url.clone() }, text.clone()),
        (Mark::Hidden, text.end..url.end + 1),
    ])
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
            if s.mark == Mark::Hidden {
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
}
