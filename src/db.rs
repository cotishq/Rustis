use std::{
    collections::{HashMap, BTreeMap},
    sync::{Arc, Mutex},
};

use bytes::Bytes;
use tokio::{
    sync::Notify,
    time::{self, Duration, Instant},
};

/// Main database handle (clonable)
pub struct Db {
    shared: Arc<Shared>,
}

/// Internal shared structure
struct Shared {
    state: Mutex<State>,
    notify: Notify,
}

/// Actual data + expiration index
struct State {
    entries: HashMap<String, Bytes>,
    expirations: BTreeMap<Instant, String>,
}

impl Db {
    /// Create a new DB and start background task
    pub fn new() -> Self {
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                entries: HashMap::new(),
                expirations: BTreeMap::new(),
            }),
            notify: Notify::new(),
        });

        // background expiration cleanup
        tokio::spawn(clean_expired(shared.clone()));

        Self { shared }
    }

    /// GET command
    pub fn get(&self, key: &str) -> Option<Bytes> {
        let state = self.shared.state.lock().unwrap();
        state.entries.get(key).cloned()
    }

    /// SET command (supports PX expiration)
    pub fn set(&self, key: String, value: Bytes, expire: Option<Duration>) {
        let mut state = self.shared.state.lock().unwrap();

        if let Some(exp) = expire {
            let when = Instant::now() + exp;
            state.expirations.insert(when, key.clone());
        }

        state.entries.insert(key, value);

        self.shared.notify.notify_one();
    }
}

/// Background task to remove expired keys
async fn clean_expired(shared: Arc<Shared>) {
    loop {
        if let Some(next_expire) = purge(&shared) {
            tokio::select! {
                _ = time::sleep_until(next_expire) => {}
                _ = shared.notify.notified() => {}
            }
        } else {
            shared.notify.notified().await;
        }
    }
}

/// Removes expired keys; returns next expiration time
fn purge(shared: &Shared) -> Option<Instant> {
    let mut state = shared.state.lock().unwrap();
    let now = Instant::now();

    loop {
        // Get next expiration WITHOUT borrowing the map
        let next = match state.expirations.iter().next() {
            Some((&when, key)) => (when, key.clone()),
            None => return None,
        };

        let (when, key) = next;

        if when > now {
            return Some(when);
        }

        // SAFE: no immutable borrow is active
        state.entries.remove(&key);
        state.expirations.remove(&when);
    }
}


/// Allow Db::default()
impl Default for Db {
    fn default() -> Self {
        Self::new()
    }
}

/// Make Db clonable (Arc clone)
impl Clone for Db {
    fn clone(&self) -> Self {
        Db {
            shared: self.shared.clone(),
        }
    }
}
