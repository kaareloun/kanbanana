//! Pool threads for network work that does not touch the store.

use std::sync::{Arc, MutexGuard};

use chrono::Utc;
use crossbeam_channel::Receiver;

use crate::error::{Error, Result};
use crate::gate::Gate;
use crate::images::{Claim, ImageState};
use crate::{Event, Shared, keys};

pub(crate) enum Job {
    Image { workspace: String, url: String },
    Upload { workspace: String, request_id: u64, filename: String, content_type: String, bytes: Vec<u8> },
    CheckKey { key: String },
    Stop,
}

pub(crate) fn run(rx: Receiver<Job>, shared: Arc<Shared>, images: Arc<ImageState>) {
    while let Ok(job) = rx.recv() {
        match job {
            Job::Image { workspace, url } => image(&shared, &images, &workspace, url),
            Job::Upload { workspace, request_id, filename, content_type, bytes } => {
                let event = match upload(&shared, &workspace, &filename, &content_type, &bytes) {
                    Ok(asset_url) => Event::Uploaded { request_id, asset_url },
                    Err(e) => {
                        eprintln!("sync: upload {filename} failed: {e}");
                        Event::UploadFailed { request_id, message: e.to_string() }
                    }
                };
                shared.out.emit(event);
            }
            Job::CheckKey { key } => shared.out.emit(Event::KeyChecked(keys::check_key(&key))),
            Job::Stop => return,
        }
    }
}

fn image(shared: &Shared, images: &ImageState, ws: &str, url: String) {
    match images.claim(&url) {
        Claim::Cached(bytes) => shared.out.emit(Event::Image { url, bytes }),
        Claim::InFlight => {}
        Claim::Fetch => {
            let got = shared.clients.get(ws).and_then(|c| Ok(c.fetch_image(&url)?));
            match got {
                Ok(bytes) => {
                    let bytes: Arc<[u8]> = bytes.into();
                    images.finish(&url, Some(bytes.clone()));
                    shared.out.emit(Event::Image { url, bytes });
                }
                Err(e) => {
                    images.finish(&url, None);
                    eprintln!("sync: image {url} failed: {e}");
                    shared.out.emit(Event::ImageFailed { url, message: e.to_string() });
                }
            }
        }
    }
}

fn upload(shared: &Shared, ws: &str, filename: &str, content_type: &str, bytes: &[u8]) -> Result<String> {
    if let Some(until) = lock_gate(shared).blocked_until(ws, Utc::now()) {
        return Err(Error::Gated(until));
    }
    let client = shared.clients.get(ws)?;
    match client.upload(filename, content_type, bytes) {
        Ok(url) => Ok(url),
        Err(e) => {
            let e = Error::from(e);
            if let Some(secs) = e.retry_after_secs() {
                let until = lock_gate(shared).block(ws, Utc::now(), secs);
                shared.out.emit(Event::RateLimited { workspace: ws.to_string(), until });
            }
            Err(e)
        }
    }
}

pub(crate) fn lock_gate(shared: &Shared) -> MutexGuard<'_, Gate> {
    shared.gate.lock().unwrap_or_else(|e| e.into_inner())
}
