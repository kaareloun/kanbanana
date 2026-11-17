//! File upload to Linear's storage and image download.

use serde::Deserialize;
use serde_json::json;
use ureq::http::Uri;

use crate::http::{RateHeaders, now_ms, plain_status_error, transport};
use crate::parse::field;
use crate::{Client, Error, Result};

const MAX_IMAGE_BYTES: u64 = 20 * 1024 * 1024;

const FILE_UPLOAD: &str = "mutation($contentType: String!, $filename: String!, $size: Int!) {
  fileUpload(contentType: $contentType, filename: $filename, size: $size) {
    success
    uploadFile { uploadUrl assetUrl headers { key value } }
  }
}";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UploadPayload {
    success: bool,
    upload_file: Option<UploadFile>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UploadFile {
    upload_url: String,
    asset_url: String,
    headers: Vec<UploadHeader>,
}

#[derive(Deserialize)]
struct UploadHeader {
    key: String,
    value: String,
}

impl Client {
    /// Uploads a file and returns its asset URL for use in Markdown.
    pub fn upload(&self, filename: &str, content_type: &str, bytes: &[u8]) -> Result<String> {
        let size = i32::try_from(bytes.len())
            .map_err(|_| Error::Http("file is too large to upload".into()))?;
        let vars = json!({ "contentType": content_type, "filename": filename, "size": size });
        let data = self.graphql(FILE_UPLOAD, vars)?;
        let payload: UploadPayload = field(data, "fileUpload")?;
        let file = match payload.upload_file {
            Some(f) if payload.success => f,
            _ => return Err(Error::GraphQl("fileUpload did not succeed".into())),
        };

        // The upload URL is a signed storage URL. It must not get the API key.
        let mut req = self
            .agent
            .put(&file.upload_url)
            .header("Content-Type", content_type)
            .header("Cache-Control", "public, max-age=31536000");
        for h in &file.headers {
            req = req.header(h.key.as_str(), h.value.as_str());
        }
        let resp = req.send(bytes).map_err(transport)?;
        let headers = RateHeaders::read(resp.headers());
        match plain_status_error(resp.status().as_u16(), &headers, now_ms()) {
            Some(e) => Err(e),
            None => Ok(file.asset_url),
        }
    }

    /// Downloads an image, at most 20 MB. Only Linear hosts get the API key.
    pub fn fetch_image(&self, url: &str) -> Result<Vec<u8>> {
        let mut req = self.agent.get(url);
        if sends_key_to(url) {
            req = req.header("Authorization", self.key.as_str());
        }
        let mut resp = req.call().map_err(transport)?;
        let headers = RateHeaders::read(resp.headers());
        if let Some(e) = plain_status_error(resp.status().as_u16(), &headers, now_ms()) {
            return Err(e);
        }
        resp.body_mut().with_config().limit(MAX_IMAGE_BYTES).read_to_vec().map_err(|e| match e {
            ureq::Error::BodyExceedsLimit(_) => Error::Http("image is larger than 20 MB".into()),
            other => transport(other),
        })
    }
}

/// True for https URLs on linear.app or its subdomains.
fn sends_key_to(url: &str) -> bool {
    let Ok(uri) = url.parse::<Uri>() else {
        return false;
    };
    let https = uri.scheme_str() == Some("https");
    let host = uri.host().unwrap_or("").to_ascii_lowercase();
    https && (host == "linear.app" || host.ends_with(".linear.app"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_only_for_linear_hosts() {
        assert!(sends_key_to("https://uploads.linear.app/a/b/c.png"));
        assert!(sends_key_to("https://linear.app/x.png"));
        assert!(!sends_key_to("http://uploads.linear.app/a.png"));
        assert!(!sends_key_to("https://evil-linear.app/a.png"));
        assert!(!sends_key_to("https://uploads.linear.app.evil.com/a.png"));
        assert!(!sends_key_to("https://example.com/a.png"));
        assert!(!sends_key_to("not a url"));
    }
}
