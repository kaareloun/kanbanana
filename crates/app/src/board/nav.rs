//! Keyboard focus moves over the cards. No painting here.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Move {
    Up,
    Down,
    Left,
    Right,
}

/// The card the focus goes to. `cols` holds the issue ids of each column the user can see,
/// top to bottom. With no `from`, or a card that is gone, the focus starts on the first card.
pub fn step(cols: &[Vec<&str>], from: Option<&str>, mv: Move) -> Option<String> {
    let at = from.and_then(|id| {
        cols.iter().enumerate().find_map(|(c, ids)| ids.iter().position(|i| *i == id).map(|r| (c, r)))
    });
    let Some((c, r)) = at else {
        return cols.iter().find_map(|ids| ids.first()).map(|s| s.to_string());
    };
    let pick = |c: usize, r: usize| Some(cols[c][r.min(cols[c].len() - 1)].to_string());
    match mv {
        Move::Down => pick(c, r + 1),
        Move::Up => pick(c, r.saturating_sub(1)),
        // Empty columns are skipped. At the edge the focus stays.
        Move::Right => match (c + 1..cols.len()).find(|&n| !cols[n].is_empty()) {
            Some(n) => pick(n, r),
            None => pick(c, r),
        },
        Move::Left => match (0..c).rev().find(|&n| !cols[n].is_empty()) {
            Some(n) => pick(n, r),
            None => pick(c, r),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cols() -> Vec<Vec<&'static str>> {
        vec![vec!["a1", "a2", "a3"], vec![], vec!["c1"], vec!["d1", "d2"]]
    }

    fn go(from: Option<&str>, mv: Move) -> Option<String> {
        step(&cols(), from, mv)
    }

    #[test]
    fn starts_on_first_card() {
        assert_eq!(go(None, Move::Down).as_deref(), Some("a1"));
        assert_eq!(go(Some("gone"), Move::Left).as_deref(), Some("a1"));
        assert_eq!(step(&[vec![], vec!["b1"]], None, Move::Up).as_deref(), Some("b1"));
        assert_eq!(step(&[vec![]], None, Move::Up), None);
    }

    #[test]
    fn up_and_down_stay_in_the_column() {
        assert_eq!(go(Some("a1"), Move::Down).as_deref(), Some("a2"));
        assert_eq!(go(Some("a3"), Move::Down).as_deref(), Some("a3"));
        assert_eq!(go(Some("a2"), Move::Up).as_deref(), Some("a1"));
        assert_eq!(go(Some("a1"), Move::Up).as_deref(), Some("a1"));
    }

    #[test]
    fn left_and_right_skip_empty_columns_and_keep_the_row() {
        assert_eq!(go(Some("a3"), Move::Right).as_deref(), Some("c1"));
        assert_eq!(go(Some("c1"), Move::Right).as_deref(), Some("d1"));
        assert_eq!(go(Some("d2"), Move::Left).as_deref(), Some("c1"));
        assert_eq!(go(Some("c1"), Move::Left).as_deref(), Some("a1"));
        assert_eq!(go(Some("d2"), Move::Right).as_deref(), Some("d2"));
        assert_eq!(go(Some("a2"), Move::Left).as_deref(), Some("a2"));
    }
}
