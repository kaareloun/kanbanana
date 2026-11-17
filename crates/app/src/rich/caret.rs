//! Caret rules: the caret never stops on hidden characters.

use super::parse::{Hide, Line, hidden, line_at};

/// Moves byte offset `cur` out of hidden text. `prev` is the caret of the last frame, so
/// the caret leaves a hidden range in the direction the user moved.
pub fn snap(lines: &[Line], prev: usize, cur: usize) -> usize {
    let mut at = cur;
    // Each step leaves one hidden range. A few steps cover runs of hidden lines.
    for _ in 0..lines.len() + 2 {
        let next = step(lines, prev, at);
        if next == at {
            break;
        }
        at = next;
    }
    at
}

fn step(lines: &[Line], prev: usize, cur: usize) -> usize {
    let forward = cur >= prev;
    let Some(li) = line_at(lines, cur) else { return cur };
    let line = &lines[li];
    for (r, kind) in hidden(std::slice::from_ref(line)) {
        match kind {
            Hide::Line => {
                let up = li.checked_sub(1);
                let down = (li + 1 < lines.len()).then(|| lines[li + 1].range.start);
                return match (forward, down, up) {
                    (true, Some(d), _) => d,
                    (false, _, Some(u)) => lines[u].range.end,
                    (_, Some(d), None) => d,
                    // The last line is hidden. The editor adds a line after it.
                    _ => line.range.end,
                };
            }
            Hide::Marker if cur >= r.start && cur < r.end => {
                // Left from the item start leaves the line. Home stays after the marker.
                let left_out = !forward && prev == r.end && cur > r.start;
                return match li.checked_sub(1) {
                    Some(u) if left_out => lines[u].range.end,
                    _ => r.end,
                };
            }
            Hide::Inline if cur > r.start && cur < r.end => return if forward { r.end } else { r.start },
            _ => {}
        }
    }
    cur
}

/// True when the caret at `at` sits on a line that is all hidden, which happens only
/// when such a line ends the text.
pub fn on_hidden_line(lines: &[Line], at: usize) -> bool {
    line_at(lines, at).is_some_and(|i| lines[i].all_hidden())
}

#[cfg(test)]
mod tests {
    use super::super::parse::parse;
    use super::*;

    fn at(t: &str, prev: usize, cur: usize) -> usize {
        snap(&parse(t, true), prev, cur)
    }

    #[test]
    fn leaves_inline_marks_in_the_move_direction() {
        let t = "a **b** c";
        // Right from "a " into the opening mark goes into the bold text.
        assert_eq!(at(t, 2, 3), 4);
        // Left from the bold text goes before the mark.
        assert_eq!(at(t, 4, 3), 2);
        // Edges stay.
        assert_eq!(at(t, 1, 2), 2);
        assert_eq!(at(t, 5, 7), 7);
    }

    #[test]
    fn stays_after_block_markers() {
        let t = "x\n- [ ] task";
        // Home or a click before the marker puts the caret after it.
        assert_eq!(at(t, 10, 2), 8);
        assert_eq!(at(t, 0, 4), 8);
        // Left from the start of the item goes to the end of the line above.
        assert_eq!(at(t, 8, 7), 1);
        // Up from below lands after the marker, not on the line above.
        assert_eq!(at("- a\n- b", 6, 1), 2);
        // A marker on the first line has no line above.
        assert_eq!(at("## H", 3, 2), 3);
    }

    #[test]
    fn skips_hidden_lines() {
        let t = "a\n![x](u)\nb";
        assert_eq!(at(t, 1, 2), 10, "down skips the image");
        assert_eq!(at(t, 10, 9), 1, "up skips the image");
        let fenced = "a\n```\ncode\n```\nb";
        assert_eq!(at(fenced, 1, 2), 6, "into the first code line");
        assert_eq!(at(fenced, 15, 14), 10, "back to the last code line");
    }

    #[test]
    fn hidden_last_line_keeps_the_caret() {
        let t = "a\n![x](u)";
        assert_eq!(at(t, 1, 2), 9);
        assert!(on_hidden_line(&parse(t, true), 9));
        assert!(!on_hidden_line(&parse(t, true), 1));
        assert_eq!(at("![x](u)\nb", 9, 3), 8, "a hidden first line goes down");
    }
}
