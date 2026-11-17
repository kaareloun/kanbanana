//! Issue comments: list, create, edit.

use model::Comment;
use serde::Deserialize;
use serde_json::json;

use crate::parse::{COMMENT_FRAGMENT, CommentNode, field};
use crate::{Client, Error, Result};

const COMMENTS_QUERY: &str = "query($id: String!, $first: Int!, $after: String) {
  issue(id: $id) {
    comments(first: $first, after: $after) {
      nodes { ...CommentFields }
      pageInfo { hasNextPage endCursor }
    }
  }
}";

const CREATE_COMMENT: &str = "mutation($input: CommentCreateInput!) {
  commentCreate(input: $input) { success comment { ...CommentFields } }
}";

const UPDATE_COMMENT: &str = "mutation($id: String!, $input: CommentUpdateInput!) {
  commentUpdate(id: $id, input: $input) { success comment { ...CommentFields } }
}";

#[derive(Deserialize)]
struct CommentPayload {
    success: bool,
    comment: Option<CommentNode>,
}

impl CommentPayload {
    fn into_comment(self, op: &str, fallback_issue_id: &str) -> Result<Comment> {
        match self.comment {
            Some(node) if self.success => Ok(node.into_comment(fallback_issue_id)),
            _ => Err(Error::GraphQl(format!("{op} did not succeed"))),
        }
    }
}

impl Client {
    /// All comments of an issue, replies included.
    pub fn comments(&self, issue_id: &str) -> Result<Vec<Comment>> {
        let query = format!("{COMMENTS_QUERY}{COMMENT_FRAGMENT}");
        let vars = json!({ "id": issue_id, "first": 100 });
        let nodes: Vec<CommentNode> = self.paginate(&query, vars, &["issue", "comments"])?;
        Ok(nodes.into_iter().map(|n| n.into_comment(issue_id)).collect())
    }

    /// `parent_id` makes the comment a reply in that thread.
    pub fn create_comment(
        &self,
        issue_id: &str,
        body: &str,
        parent_id: Option<&str>,
    ) -> Result<Comment> {
        let query = format!("{CREATE_COMMENT}{COMMENT_FRAGMENT}");
        let mut input = json!({ "issueId": issue_id, "body": body });
        if let Some(p) = parent_id {
            input["parentId"] = json!(p);
        }
        let data = self.graphql(&query, json!({ "input": input }))?;
        let payload: CommentPayload = field(data, "commentCreate")?;
        payload.into_comment("commentCreate", issue_id)
    }

    pub fn update_comment(&self, id: &str, body: &str) -> Result<Comment> {
        let query = format!("{UPDATE_COMMENT}{COMMENT_FRAGMENT}");
        let data = self.graphql(&query, json!({ "id": id, "input": { "body": body } }))?;
        let payload: CommentPayload = field(data, "commentUpdate")?;
        payload.into_comment("commentUpdate", "")
    }
}
