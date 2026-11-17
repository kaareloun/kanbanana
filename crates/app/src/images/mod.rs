//! Image bytes for http(s) URIs. The loader never fetches. It records the URLs that egui
//! asks for, and the app fetches them on its worker and pushes the bytes back in.

pub mod mock;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use eframe::egui::{
    self, Id,
    load::{Bytes, BytesLoadResult, BytesLoader, BytesPoll, LoadError},
};

pub use mock::mock_bytes;

enum Entry {
    Requested,
    Ready(Bytes),
    Failed(String),
}

#[derive(Default)]
struct Store {
    entries: Mutex<HashMap<String, Entry>>,
    queue: Mutex<Vec<String>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

struct Loader(Arc<Store>);

impl BytesLoader for Loader {
    fn id(&self) -> &str {
        egui::generate_loader_id!(Loader)
    }

    fn load(&self, _ctx: &egui::Context, uri: &str) -> BytesLoadResult {
        if !(uri.starts_with("https://") || uri.starts_with("http://")) {
            return Err(LoadError::NotSupported);
        }
        let mut entries = lock(&self.0.entries);
        match entries.get(uri) {
            Some(Entry::Ready(bytes)) => Ok(BytesPoll::Ready { size: None, bytes: bytes.clone(), mime: None }),
            Some(Entry::Failed(msg)) => Err(LoadError::Loading(msg.clone())),
            Some(Entry::Requested) => Ok(BytesPoll::Pending { size: None }),
            None => {
                entries.insert(uri.to_string(), Entry::Requested);
                lock(&self.0.queue).push(uri.to_string());
                Ok(BytesPoll::Pending { size: None })
            }
        }
    }

    fn forget(&self, uri: &str) {
        lock(&self.0.entries).remove(uri);
    }

    fn forget_all(&self) {
        lock(&self.0.entries).clear();
    }

    fn byte_size(&self) -> usize {
        lock(&self.0.entries)
            .values()
            .map(|e| match e {
                Entry::Ready(b) => b.len(),
                _ => 0,
            })
            .sum()
    }

    fn has_pending(&self) -> bool {
        lock(&self.0.entries).values().any(|e| matches!(e, Entry::Requested))
    }
}

fn store_id() -> Id {
    Id::unique("lb-images-store")
}

fn store(ctx: &egui::Context) -> Option<Arc<Store>> {
    ctx.data(|d| d.get_temp::<Arc<Store>>(store_id()))
}

/// Installs the decoders and the http(s) bytes loader. Call once at startup.
pub fn install(ctx: &egui::Context) {
    egui_extras::install_image_loaders(ctx);
    if store(ctx).is_some() {
        return;
    }
    let s = Arc::new(Store::default());
    ctx.data_mut(|d| d.insert_temp(store_id(), s.clone()));
    ctx.add_bytes_loader(Arc::new(Loader(s)));
}

/// URLs that egui asked for since the last call. The app fetches each one once.
pub fn take_requests(ctx: &egui::Context) -> Vec<String> {
    store(ctx).map(|s| std::mem::take(&mut *lock(&s.queue))).unwrap_or_default()
}

pub fn insert(ctx: &egui::Context, url: &str, bytes: Vec<u8>) {
    let Some(s) = store(ctx) else { return };
    // The decoders may hold an error from an earlier failed try. This also clears our entry.
    ctx.forget_image(url);
    lock(&s.entries).insert(url.to_string(), Entry::Ready(Bytes::Shared(bytes.into())));
    ctx.request_repaint();
}

pub fn failed(ctx: &egui::Context, url: &str, message: &str) {
    let Some(s) = store(ctx) else { return };
    lock(&s.entries).insert(url.to_string(), Entry::Failed(message.to_string()));
    ctx.request_repaint();
}
