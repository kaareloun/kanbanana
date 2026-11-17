//! GraphQL transport and the mapping from HTTP and GraphQL failures to `Error`.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use ureq::http::HeaderMap;

use crate::parse::{Connection, field};
use crate::{Client, ENDPOINT, Error, Result};

/// Big issue pages with long descriptions can pass ureq's 10 MB default.
const MAX_RESPONSE_BYTES: u64 = 64 * 1024 * 1024;
const DEFAULT_RETRY_SECS: u64 = 60;

impl Client {
    /// Posts one GraphQL operation and returns its `data` object.
    pub(crate) fn graphql(&self, query: &str, variables: Value) -> Result<Value> {
        let body = json!({ "query": query, "variables": variables });
        let mut resp = self
            .agent
            .post(ENDPOINT)
            .header("Authorization", self.key.as_str())
            .send_json(&body)
            .map_err(transport)?;
        let status = resp.status().as_u16();
        let headers = RateHeaders::read(resp.headers());
        if let Some(n) = headers.remaining {
            self.set_rate_limit_remaining(n);
        }
        let text = resp
            .body_mut()
            .with_config()
            .limit(MAX_RESPONSE_BYTES)
            .read_to_string()
            .map_err(transport)?;
        map_response(status, &headers, &text, now_ms())
    }

    /// Reads every page of the connection at `path` in `data`.
    /// The query takes `$after: String` and selects `pageInfo { hasNextPage endCursor }`.
    pub(crate) fn paginate<T: DeserializeOwned>(
        &self,
        query: &str,
        mut variables: Value,
        path: &[&str],
    ) -> Result<Vec<T>> {
        let mut out = Vec::new();
        loop {
            let data = self.graphql(query, variables.clone())?;
            let conn: Connection<T> = connection_at(data, path)?;
            let next = conn.next_cursor();
            out.extend(conn.nodes);
            match next {
                Some(cursor) => variables["after"] = Value::String(cursor),
                None => return Ok(out),
            }
        }
    }
}

fn connection_at<T: DeserializeOwned>(mut data: Value, path: &[&str]) -> Result<Connection<T>> {
    let Some((last, parents)) = path.split_last() else {
        return Err(Error::Decode("empty connection path".into()));
    };
    for key in parents {
        data = match data.get_mut(*key).map(Value::take) {
            Some(v) if !v.is_null() => v,
            _ => return Err(Error::Decode(format!("missing field `{key}`"))),
        };
    }
    field(data, last)
}

/// Rate limit headers of one response.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct RateHeaders {
    pub remaining: Option<i64>,
    /// Epoch milliseconds when the request budget resets.
    pub reset_ms: Option<i64>,
    pub retry_after_secs: Option<u64>,
}

impl RateHeaders {
    pub fn read(headers: &HeaderMap) -> RateHeaders {
        let get = |name: &str| {
            headers
                .get(name)
                .and_then(|v| v.to_str().ok())
                .map(|s| s.trim().to_string())
        };
        RateHeaders {
            remaining: get("x-ratelimit-requests-remaining").and_then(|s| s.parse().ok()),
            reset_ms: get("x-ratelimit-requests-reset").and_then(|s| s.parse().ok()),
            retry_after_secs: get("retry-after").and_then(|s| s.parse().ok()),
        }
    }

    pub fn retry_after(&self, now_ms: i64) -> u64 {
        if let Some(reset) = self.reset_ms {
            let wait_ms = (reset - now_ms).max(0) as u64;
            return wait_ms.div_ceil(1000).max(1);
        }
        self.retry_after_secs.unwrap_or(DEFAULT_RETRY_SECS)
    }
}

pub(crate) fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub(crate) fn transport(e: ureq::Error) -> Error {
    Error::Http(e.to_string())
}

/// Maps a GraphQL HTTP response to its `data` object or to an `Error`.
pub(crate) fn map_response(
    status: u16,
    headers: &RateHeaders,
    body: &str,
    now_ms: i64,
) -> Result<Value> {
    if status == 401 || status == 403 {
        return Err(Error::Auth);
    }
    if status == 429 {
        return Err(Error::RateLimited { retry_after_secs: headers.retry_after(now_ms) });
    }
    let ok = (200..300).contains(&status);
    let mut json: Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(e) if ok => return Err(Error::Decode(e.to_string())),
        Err(_) => return Err(Error::Http(format!("status {status}: {}", snippet(body)))),
    };
    // Linear sends GraphQL errors with HTTP 400 too, so read them before the status.
    if let Some(errors) = json.get("errors").and_then(Value::as_array).filter(|e| !e.is_empty()) {
        return Err(graphql_error(errors, headers, now_ms));
    }
    if !ok {
        return Err(Error::Http(format!("status {status}: {}", snippet(body))));
    }
    match json.get_mut("data").map(Value::take) {
        Some(data) if data.is_object() => Ok(data),
        _ => Err(Error::Decode("response has no data".into())),
    }
}

fn graphql_error(errors: &[Value], headers: &RateHeaders, now_ms: i64) -> Error {
    let has_code = |wanted: &str| {
        errors.iter().any(|e| {
            e.pointer("/extensions/code").and_then(Value::as_str) == Some(wanted)
        })
    };
    if has_code("RATELIMITED") {
        return Error::RateLimited { retry_after_secs: headers.retry_after(now_ms) };
    }
    if has_code("AUTHENTICATION_ERROR") {
        return Error::Auth;
    }
    let messages: Vec<&str> =
        errors.iter().filter_map(|e| e.get("message").and_then(Value::as_str)).collect();
    if messages.is_empty() {
        Error::GraphQl("unknown error".into())
    } else {
        Error::GraphQl(messages.join("; "))
    }
}

/// Maps a non-GraphQL status (upload PUT, image GET) to an error, or None for 2xx.
pub(crate) fn plain_status_error(status: u16, headers: &RateHeaders, now_ms: i64) -> Option<Error> {
    match status {
        200..=299 => None,
        429 => Some(Error::RateLimited { retry_after_secs: headers.retry_after(now_ms) }),
        _ => Some(Error::Http(format!("status {status}"))),
    }
}

fn snippet(body: &str) -> String {
    body.chars().take(200).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_700_000_000_000;

    #[test]
    fn data_is_returned() {
        let v = map_response(200, &RateHeaders::default(), r#"{"data":{"a":1}}"#, NOW).unwrap();
        assert_eq!(v["a"], 1);
    }

    #[test]
    fn ratelimited_code_uses_reset_header() {
        let body = r#"{"errors":[{"message":"Rate limit exceeded",
            "extensions":{"code":"RATELIMITED"}}],"data":null}"#;
        let headers = RateHeaders { reset_ms: Some(NOW + 12_300), ..Default::default() };
        let err = map_response(400, &headers, body, NOW).unwrap_err();
        assert!(matches!(err, Error::RateLimited { retry_after_secs: 13 }), "{err:?}");
    }

    #[test]
    fn http_429_falls_back_to_retry_after_then_default() {
        let headers = RateHeaders { retry_after_secs: Some(7), ..Default::default() };
        let err = map_response(429, &headers, "", NOW).unwrap_err();
        assert!(matches!(err, Error::RateLimited { retry_after_secs: 7 }));
        let err = map_response(429, &RateHeaders::default(), "", NOW).unwrap_err();
        assert!(matches!(err, Error::RateLimited { retry_after_secs: 60 }));
    }

    #[test]
    fn auth_errors() {
        let err = map_response(401, &RateHeaders::default(), "", NOW).unwrap_err();
        assert!(matches!(err, Error::Auth));
        let body = r#"{"errors":[{"message":"Authentication required",
            "extensions":{"code":"AUTHENTICATION_ERROR"}}]}"#;
        let err = map_response(400, &RateHeaders::default(), body, NOW).unwrap_err();
        assert!(matches!(err, Error::Auth));
    }

    #[test]
    fn other_graphql_errors_join_messages() {
        let body = r#"{"errors":[{"message":"bad a"},{"message":"bad b"}]}"#;
        let err = map_response(200, &RateHeaders::default(), body, NOW).unwrap_err();
        match err {
            Error::GraphQl(m) => assert_eq!(m, "bad a; bad b"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn non_json_error_is_http() {
        let err = map_response(502, &RateHeaders::default(), "<html>", NOW).unwrap_err();
        assert!(matches!(err, Error::Http(_)));
        let err = map_response(200, &RateHeaders::default(), "<html>", NOW).unwrap_err();
        assert!(matches!(err, Error::Decode(_)));
    }

    #[test]
    fn rate_headers_parse() {
        let mut h = HeaderMap::new();
        h.insert("X-RateLimit-Requests-Remaining", "1499".parse().unwrap());
        h.insert("X-RateLimit-Requests-Reset", "1700000005000".parse().unwrap());
        let r = RateHeaders::read(&h);
        assert_eq!(r.remaining, Some(1499));
        assert_eq!(r.retry_after(NOW), 5);
    }
}
