use std::{
    collections::{BTreeMap, HashMap}, sync::{Arc, Mutex}
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
    lists : HashMap<String , Vec<Bytes>>,
    expirations: BTreeMap<Instant, String>,
}

impl Db {
    /// Create a new DB and start background task
    pub fn new() -> Self {
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                entries: HashMap::new(),
                lists : HashMap::new(),
                expirations: BTreeMap::new(),
            }),
            notify: Notify::new(),
        });

        tokio::spawn(clean_expired(shared.clone()));

        Self { shared }
    }

    /// GET command
    pub fn get(&self, key: &str) -> Option<Bytes> {
        let mut state = self.shared.state.lock().unwrap();
        let now = Instant::now();
        
        // Check if key has expired
        let mut expired_instant = None;
        for (&when, k) in state.expirations.iter() {
            if k == key {
                if when <= now {
                    // Key has expired
                    expired_instant = Some(when);
                    break;
                } else {
                    // Key exists and is not expired, return it
                    return state.entries.get(key).cloned();
                }
            }
        }
        
        // Remove expired key if found
        if let Some(when) = expired_instant {
            state.entries.remove(key);
            state.expirations.remove(&when);
            return None;
        }
        
        // Key doesn't have an expiration, return it if it exists
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

    pub fn rpush(&self , key: String , values: Vec<Bytes>) -> usize {
        let mut state = self.shared.state.lock().unwrap();

        let list = state.lists.entry(key).or_insert_with(Vec::new);

        for value in values  {
            list.push(value);
            
        }

        let len = list.len() ;
        self.shared.notify.notify_one();
        len
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

        state.entries.remove(&key);
        state.expirations.remove(&when);
    }
}


impl Default for Db {
    fn default() -> Self {
        Self::new()
    }
}


impl Clone for Db {
    fn clone(&self) -> Self {
        Db {
            shared: self.shared.clone(),
        }
    }
}
