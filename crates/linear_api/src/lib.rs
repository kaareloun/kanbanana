//! Blocking Linear GraphQL client: sync reads, mutations, uploads and image fetch.
//! Callers run every call on a worker thread.

mod comments;
mod files;
mod http;
mod issues;
mod parse;
mod sync;

use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use model::{Issue, Label, Project, Team, User, WorkflowState};

pub use chrono::{DateTime, Utc};

const ENDPOINT: &str = "https://api.linear.app/graphql";
const USER_AGENT: &str = "kanbanana/0.1";

/// Marks "no value seen yet" in `Client::rate_limit_remaining`.
const NO_RATE_LIMIT: i64 = i64::MIN;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Linear rejected the API key")]
    Auth,
    #[error("Linear rate limit hit, retry in {retry_after_secs} s")]
    RateLimited { retry_after_secs: u64 },
    #[error("Linear GraphQL error: {0}")]
    GraphQl(String),
    #[error("HTTP error: {0}")]
    Http(String),
    #[error("cannot decode Linear response: {0}")]
    Decode(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// The owner of the API key.
#[derive(Clone, Debug, PartialEq)]
pub struct Viewer {
    pub id: String,
    pub name: String,
    pub organization_name: String,
}

/// Teams, states, projects, users and labels of one workspace.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Catalog {
    pub teams: Vec<Team>,
    pub states: Vec<WorkflowState>,
    pub projects: Vec<Project>,
    pub users: Vec<User>,
    pub labels: Vec<Label>,
}

/// Result of one sync run. The caller deletes `archived_ids` from the cache.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct IssuePage {
    pub issues: Vec<Issue>,
    pub archived_ids: Vec<String>,
}

pub struct Client {
    key: String,
    agent: ureq::Agent,
    rate_limit_remaining: AtomicI64,
}

impl Client {
    pub fn new(key: impl Into<String>) -> Client {
        let config = ureq::Agent::config_builder()
            // Status codes are mapped to `Error` by hand, so 4xx bodies stay readable.
            .http_status_as_error(false)
            .user_agent(USER_AGENT)
            .timeout_connect(Some(Duration::from_secs(15)))
            .timeout_global(Some(Duration::from_secs(120)))
            .build();
        Client {
            key: key.into(),
            agent: config.into(),
            rate_limit_remaining: AtomicI64::new(NO_RATE_LIMIT),
        }
    }

    /// Last `X-RateLimit-Requests-Remaining` value Linear sent, if any.
    pub fn rate_limit_remaining(&self) -> Option<i64> {
        match self.rate_limit_remaining.load(Ordering::Relaxed) {
            NO_RATE_LIMIT => None,
            n => Some(n),
        }
    }

    fn set_rate_limit_remaining(&self, n: i64) {
        self.rate_limit_remaining.store(n, Ordering::Relaxed);
    }
}
