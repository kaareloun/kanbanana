//! Demo states for `--mock --screen panel*`, matching designs v3 03 and 05.

use model::{BoardData, Issue};

use super::{DescMode, PanelAction, PanelState, composer, on_upload_progress};
use crate::actions;

/// WEB-214 description from design 05.
pub const MOCK_DESCRIPTION: &str = "When a customer gets a **partial refund**, the payment provider sends two webhooks. \
We handle both, and the second one flips the order back to paid.

## Steps
1. Pay an order with a test card in the sandbox
2. Refund 20 EUR from the admin
3. After about 5 s the order shows Paid again

- [ ] Ignore `order.updated` when a refund exists
- [x] Add a test for partial refunds

![admin-order-4471.png](https://uploads.linear.app/mock/admin-order-4471.png)";

/// Sets up the panel for a screenshot. `screen` is `panel`, `panel-comments` or `panel-edit`.
pub fn demo(state: &mut PanelState, issue: &Issue, _data: &BoardData, screen: &str) {
    state.issue_id = Some(issue.id.clone());
    state.reset_for(&issue.id);
    match screen {
        "panel" => state.pending.push(PanelAction::CopyText(actions::copy_prompt(issue))),
        "panel-comments" => {
            state.scroll_to_comments = true;
            composer::set_draft_for_mock(
                &mut state.composer,
                &issue.id,
                "Pushed the fix. A refund now keeps the order in Refunded:",
                Some("paste-0951.png"),
            );
        }
        "panel-edit" => {
            let request_id = 9000;
            let buffer = format!("{MOCK_DESCRIPTION}\n![Uploading paste-0942.png...]()");
            let last = "partial refunds";
            let cursor = buffer.find(last).map(|i| buffer[..i + last.len()].chars().count());
            // Selects words, so the floating toolbar shows over them.
            if let Some(i) = buffer.find("for partial refunds").map(|i| i + 4) {
                super::description::demo_select(buffer[..i].chars().count(), buffer[..i + 14].chars().count());
            }
            state.desc = DescMode::Edit { buffer, preview: false };
            state.desc_focus = true;
            state.desc_cursor = cursor;
            state.track_upload(request_id, "paste-0942.png", true);
            on_upload_progress(state, request_id, 0.62);
        }
        _ => {}
    }
}
