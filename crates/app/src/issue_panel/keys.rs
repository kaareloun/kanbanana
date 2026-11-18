//! Panel shortcuts. Runs before the panel draws, so it can take keys away from text edits.
//! It consumes the keys it uses, so the board does not act on the same press.

use eframe::egui::{self, InputState, Key, Modifiers};
use model::{BoardData, Issue, IssueChange, Priority};

use super::{DESC_ID, PanelAction, PanelState, PickerKind, id};

/// One-key commands on an issue. The panel runs them on the open issue, the board on the
/// card with the keyboard focus.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum IssueKey {
    Picker(PickerKind),
    /// `I`: assign to me, or unassign when it is mine.
    AssignMe,
    Priority(Priority),
}

const ISSUE_KEYS: [(Key, IssueKey); 10] = [
    (Key::S, IssueKey::Picker(PickerKind::Status)),
    (Key::P, IssueKey::Picker(PickerKind::Priority)),
    (Key::A, IssueKey::Picker(PickerKind::Assignee)),
    (Key::L, IssueKey::Picker(PickerKind::Labels)),
    (Key::I, IssueKey::AssignMe),
    (Key::Num1, IssueKey::Priority(Priority::Urgent)),
    (Key::Num2, IssueKey::Priority(Priority::High)),
    (Key::Num3, IssueKey::Priority(Priority::Medium)),
    (Key::Num4, IssueKey::Priority(Priority::Low)),
    (Key::Num0, IssueKey::Priority(Priority::None)),
];

/// Takes the first issue key of this frame out of the input. Call it only when no text
/// field has focus.
pub fn take_issue_key(ctx: &egui::Context) -> Option<IssueKey> {
    ctx.input_mut(|i| ISSUE_KEYS.iter().find(|(k, _)| take_plain(i, *k)).map(|(_, a)| *a))
}

/// Consumes `k` when it is pressed without any modifier. `consume_key` alone ignores
/// Shift and Alt, so Alt+1 would match 1.
pub fn take_plain(i: &mut InputState, k: Key) -> bool {
    !i.modifiers.any() && i.consume_key(Modifiers::NONE, k)
}

/// The change `I` makes: assign to the key owner, or unassign when the issue is theirs.
pub fn assign_me(issue: &Issue, data: &BoardData) -> Option<IssueChange> {
    let me = data.viewer_id.clone()?;
    let next = if issue.assignee_id.as_ref() == Some(&me) { None } else { Some(me) };
    Some(IssueChange::Assignee(next))
}

/// The change a priority key makes. None when the issue has that priority already.
pub fn set_priority(issue: &Issue, p: Priority) -> Option<IssueChange> {
    (issue.priority != p).then_some(IssueChange::Priority(p))
}

pub fn handle(ctx: &egui::Context, state: &mut PanelState, issue: &Issue, data: &BoardData, out: &mut Vec<PanelAction>) {
    // The open picker reads Up, Down, Enter and Esc itself.
    if state.picker.is_some() || state.keys_off {
        return;
    }
    let focused = ctx.memory(|m| m.focused());
    let typing = ctx.text_edit_focused();

    if ctx.input(|i| i.key_pressed(Key::Escape)) {
        let editor = if typing { focused } else { state.last_focus };
        let typing = editor.is_some();
        if state.title_edit.is_some() {
            take(ctx, Modifiers::NONE, Key::Escape);
            state.title_edit = None;
            surrender(ctx, focused);
        } else if editor == Some(id(DESC_ID)) {
            // Esc only blurs the description. It saves on focus loss.
            take(ctx, Modifiers::NONE, Key::Escape);
            surrender(ctx, focused);
        } else if !typing {
            take(ctx, Modifiers::NONE, Key::Escape);
            out.push(PanelAction::Close);
        }
        // Otherwise a comment editor or the composer has focus and handles Esc itself.
    }

    // The board handles Cmd+C and Cmd+Shift+C for the selected issue.
    if typing {
        return;
    }
    if ctx.input_mut(|i| take_plain(i, Key::E)) {
        state.desc_focus = true;
    }
    let change = match take_issue_key(ctx) {
        Some(IssueKey::Picker(kind)) => {
            state.open_picker(kind, issue, data);
            None
        }
        Some(IssueKey::AssignMe) => assign_me(issue, data),
        Some(IssueKey::Priority(p)) => set_priority(issue, p),
        None => None,
    };
    if let Some(c) = change {
        out.push(PanelAction::Change(c));
    }
}

fn take(ctx: &egui::Context, m: Modifiers, k: Key) -> bool {
    ctx.input_mut(|i| i.consume_key(m, k))
}

fn surrender(ctx: &egui::Context, focused: Option<egui::Id>) {
    if let Some(f) = focused {
        ctx.memory_mut(|m| m.surrender_focus(f));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn issue(assignee: Option<&str>) -> Issue {
        let t = chrono::Utc::now();
        Issue {
            id: "i1".into(),
            source: model::Source::Linear,
            workspace: "acme".into(),
            identifier: "WEB-1".into(),
            title: "T".into(),
            description: String::new(),
            team_id: "t".into(),
            state_id: "s".into(),
            project_id: None,
            assignee_id: assignee.map(Into::into),
            priority: Priority::High,
            label_ids: vec![],
            sort_order: 0.0,
            comment_count: 0,
            url: None,
            created_at: t,
            updated_at: t,
        }
    }

    #[test]
    fn assign_me_toggles() {
        let data = BoardData { viewer_id: Some("me".into()), ..Default::default() };
        assert_eq!(assign_me(&issue(None), &data), Some(IssueChange::Assignee(Some("me".into()))));
        assert_eq!(assign_me(&issue(Some("other")), &data), Some(IssueChange::Assignee(Some("me".into()))));
        assert_eq!(assign_me(&issue(Some("me")), &data), Some(IssueChange::Assignee(None)));
        assert_eq!(assign_me(&issue(None), &BoardData::default()), None);
    }

    #[test]
    fn same_priority_is_no_change() {
        assert_eq!(set_priority(&issue(None), Priority::High), None);
        assert_eq!(set_priority(&issue(None), Priority::Urgent), Some(IssueChange::Priority(Priority::Urgent)));
    }
}
