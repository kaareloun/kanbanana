//! First-run flow: pick a theme, link the first workspace, connect Claude Code.

pub mod claude;
pub mod page;
mod preview;
pub mod theme_grid;

use claude::ClaudeLink;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Step {
    Theme,
    Workspace,
    Claude,
}

impl Step {
    pub const ALL: [Step; 3] = [Step::Theme, Step::Workspace, Step::Claude];

    fn index(self) -> usize {
        self as usize
    }
}

/// What the app does with the flow at startup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Start {
    Show,
    /// Existing users skip the flow. The app only writes the flag.
    MarkDone,
    Skip,
}

pub fn start(onboarded: bool, has_workspaces: bool) -> Start {
    match (onboarded, has_workspaces) {
        (true, _) => Start::Skip,
        (false, true) => Start::MarkDone,
        (false, false) => Start::Show,
    }
}

pub struct OnboardingState {
    pub step: Step,
    /// -1 when the last move went back, 1 when it went forward. The new step slides in from that side.
    dir: f32,
    /// egui time of the last step change.
    changed_at: f64,
    /// Name of the workspace this run linked.
    pub linked: Option<String>,
    pub claude: ClaudeLink,
}

impl Default for OnboardingState {
    fn default() -> Self {
        OnboardingState::at(Step::Theme)
    }
}

impl OnboardingState {
    /// The flow on `step`, with no slide in.
    pub fn at(step: Step) -> Self {
        OnboardingState { step, dir: 1.0, changed_at: f64::NEG_INFINITY, linked: None, claude: ClaudeLink::default() }
    }

    pub fn go(&mut self, step: Step, now: f64) {
        if step != self.step {
            self.dir = if step > self.step { 1.0 } else { -1.0 };
            self.step = step;
            self.changed_at = now;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shows_only_on_a_fresh_install() {
        assert_eq!(start(false, false), Start::Show);
        assert_eq!(start(false, true), Start::MarkDone);
        assert_eq!(start(true, false), Start::Skip);
        assert_eq!(start(true, true), Start::Skip);
    }

    #[test]
    fn go_sets_the_slide_side() {
        let mut s = OnboardingState::default();
        s.go(Step::Claude, 1.0);
        assert_eq!((s.step, s.dir, s.changed_at), (Step::Claude, 1.0, 1.0));
        s.go(Step::Workspace, 2.0);
        assert_eq!((s.step, s.dir), (Step::Workspace, -1.0));
        s.go(Step::Workspace, 3.0);
        assert_eq!(s.changed_at, 2.0);
    }
}
