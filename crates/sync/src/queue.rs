//! The worker's job queue. Queued syncs of one workspace merge into one.

use std::collections::VecDeque;

use crate::Command;

#[derive(Debug, PartialEq)]
pub(crate) enum Job {
    /// `user` is true when the user asked for it. Only those wait out a rate limit.
    Sync { workspace: String, user: bool },
    Cmd(Command),
}

pub(crate) fn push(queue: &mut VecDeque<Job>, cmd: Command) {
    match cmd {
        Command::SyncNow { workspace } => push_sync(queue, &workspace, true),
        Command::RemoveWorkspace(ws) => {
            queue.retain(|j| !matches!(j, Job::Sync { workspace, .. } if *workspace == ws));
            queue.push_back(Job::Cmd(Command::RemoveWorkspace(ws)));
        }
        // Syncs are not worth the wait at exit. Queued edits still run.
        Command::Shutdown => {
            queue.retain(|j| !matches!(j, Job::Sync { .. }));
            queue.push_back(Job::Cmd(Command::Shutdown));
        }
        other => queue.push_back(Job::Cmd(other)),
    }
}

pub(crate) fn push_sync(queue: &mut VecDeque<Job>, ws: &str, user: bool) {
    for job in queue.iter_mut() {
        if let Job::Sync { workspace, user: queued_user } = job
            && workspace == ws
        {
            *queued_user |= user;
            return;
        }
    }
    queue.push_back(Job::Sync { workspace: ws.to_string(), user });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sync(ws: &str) -> Command {
        Command::SyncNow { workspace: ws.into() }
    }

    #[test]
    fn queued_syncs_of_one_workspace_merge() {
        let mut q = VecDeque::new();
        push_sync(&mut q, "a", false);
        push(&mut q, sync("b"));
        push(&mut q, sync("a"));
        push(&mut q, Command::SetActive(None));
        push(&mut q, sync("b"));
        push(&mut q, sync("a"));
        assert_eq!(
            q.into_iter().collect::<Vec<_>>(),
            vec![
                Job::Sync { workspace: "a".into(), user: true },
                Job::Sync { workspace: "b".into(), user: true },
                Job::Cmd(Command::SetActive(None)),
            ]
        );
    }

    #[test]
    fn timer_sync_does_not_downgrade_user_sync() {
        let mut q = VecDeque::new();
        push(&mut q, sync("a"));
        push_sync(&mut q, "a", false);
        assert_eq!(q.pop_front(), Some(Job::Sync { workspace: "a".into(), user: true }));
    }

    #[test]
    fn remove_and_shutdown_drop_queued_syncs() {
        let mut q = VecDeque::new();
        push(&mut q, sync("a"));
        push(&mut q, sync("b"));
        push(&mut q, Command::RemoveWorkspace("a".into()));
        assert_eq!(q.len(), 2);
        assert_eq!(q[0], Job::Sync { workspace: "b".into(), user: true });
        push(&mut q, Command::Shutdown);
        assert_eq!(
            q.into_iter().collect::<Vec<_>>(),
            vec![Job::Cmd(Command::RemoveWorkspace("a".into())), Job::Cmd(Command::Shutdown)]
        );
    }
}
