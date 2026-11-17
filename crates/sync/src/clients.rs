//! One Linear client per workspace, made from the stored key on first use.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use linear_api::Client;

use crate::error::{Error, Result};

#[derive(Default)]
pub(crate) struct Clients {
    map: Mutex<HashMap<String, Arc<Client>>>,
}

impl Clients {
    pub fn get(&self, ws: &str) -> Result<Arc<Client>> {
        if let Some(c) = self.lock().get(ws) {
            return Ok(c.clone());
        }
        // Read the key file without the lock.
        let key = store::secrets::get_api_key(ws)?.ok_or_else(|| Error::NoKey(ws.to_string()))?;
        let client = self.lock().entry(ws.to_string()).or_insert_with(|| Arc::new(Client::new(key))).clone();
        Ok(client)
    }

    pub fn insert(&self, ws: &str, client: Client) {
        self.lock().insert(ws.to_string(), Arc::new(client));
    }

    pub fn remove(&self, ws: &str) {
        self.lock().remove(ws);
    }

    /// The client when it exists already. Does not read the key file.
    pub fn cached(&self, ws: &str) -> Option<Arc<Client>> {
        self.lock().get(ws).cloned()
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<String, Arc<Client>>> {
        // A panic while the lock is held leaves the map whole. Keep using it.
        self.map.lock().unwrap_or_else(|e| e.into_inner())
    }
}
