use std::collections::{HashMap, VecDeque, BTreeMap};
use std::sync::{Arc, Mutex};
use bytes::Bytes;
use tokio::sync::Notify;
use tokio::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub enum DbValue {
    String(Bytes),
    List(VecDeque<Bytes>),
}

/// Main database handle (clonable)
#[derive(Clone)]
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
    data: HashMap<String, DbValue>,
    expirations: BTreeMap<Instant, String>,
}

impl Db {
    /// Create a new DB and start background task
    pub fn new() -> Self {
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                data: HashMap::new(),
                expirations: BTreeMap::new(),
            }),
            notify: Notify::new(),
        });

        tokio::spawn(clean_expired(shared.clone()));

        Self { shared }
    }

    /// Remove expired keys; returns next expiration time
    fn remove_expired_key(&self, key: &str) {
        let mut state = self.shared.state.lock().unwrap();
        state.data.remove(key);
    }

    /// Check if a key has expired
    fn is_expired(&self, key: &str) -> bool {
        let state = self.shared.state.lock().unwrap();
        let now = Instant::now();

        state
            .expirations
            .iter()
            .any(|(&when, k)| k == key && when <= now)
    }

    /// Remove expiration entry for a key
    fn remove_expiration(&self, key: &str) {
        let mut state = self.shared.state.lock().unwrap();
        if let Some((&when, _)) = state.expirations.iter().find(|(_, k)| *k == key) {
            state.expirations.remove(&when);
        }
    }

    /// Store a string value
    pub fn set_string(&self, key: String, value: Bytes, expire: Option<Duration>) {
        let mut state = self.shared.state.lock().unwrap();

        if let Some(exp) = expire {
            let when = Instant::now() + exp;
            state.expirations.insert(when, key.clone());
        }

        state.data.insert(key, DbValue::String(value));
        self.shared.notify.notify_one();
    }

    /// Get a string value
    pub fn get_string(&self, key: &str) -> Option<Bytes> {
        if self.is_expired(key) {
            self.remove_expired_key(key);
            self.remove_expiration(key);
            return None;
        }

        let state = self.shared.state.lock().unwrap();
        match state.data.get(key) {
            Some(DbValue::String(bytes)) => Some(bytes.clone()),
            _ => None,
        }
    }

    /// Append elements to a list
    pub fn rpush(&self, key: String, values: Vec<Bytes>) -> usize {
        let mut state = self.shared.state.lock().unwrap();

        let list = state
            .data
            .entry(key)
            .or_insert_with(|| DbValue::List(VecDeque::new()));

        if let DbValue::List(deque) = list {
            for value in values {
                deque.push_back(value);
            }
            let len = deque.len();
            self.shared.notify.notify_one();
            len
        } else {
            panic!("Key is not a list");
        }
    }

    pub fn lpush(&self , key: String , values: Vec<Bytes>) -> usize {
        let mut state = self.shared.state.lock().unwrap();

        let list = state
            .data
            .entry(key)
            .or_insert_with(|| DbValue::List(VecDeque::new()));

        if let DbValue::List(deque) = list{
            for value in values {
                deque.push_front(value);
            }

            let len = deque.len();
            self.shared.notify.notify_one();
            len
        } else {
            panic!("key is not a list");
        }
    }

    pub fn llen(&self , key: String) -> usize{
        let state = self.shared.state.lock().unwrap();

        match state.data.get(&key) {
            Some(DbValue::List(deque)) => deque.len(),
            _ => 0,
            
        }
    }

    /// Get a range of elements from a list
    pub fn lrange(&self, key: &str, start: i64, end: i64) -> Vec<Bytes> {
        let state = self.shared.state.lock().unwrap();

        match state.data.get(key) {
            Some(DbValue::List(deque)) => {
                let len = deque.len() as i64;
                let start = Self::normalize_index(start, len) as usize;
                let end = Self::normalize_index(end, len) as usize;

                if start > end || start >= deque.len() {
                    return vec![];
                }

                deque
                    .iter()
                    .skip(start)
                    .take(end - start + 1)
                    .cloned()
                    .collect()
            }
            _ => vec![],
        }
    } 

    pub fn lpop(&self , key: String) -> Option<Bytes>{
        let mut state = self.shared.state.lock().unwrap();

        match state.data.get_mut(&key) {
            Some(DbValue::List(deque))=> {
                let popped = deque.pop_front();

                if deque.is_empty() {
                    state.data.remove(&key);
                }

                popped
            }

            _ => None,
            
        }
    }

    pub fn lpop_n(&self, key: String, count: usize) -> Vec<Bytes> {
        let mut state = self.shared.state.lock().unwrap();

        let list = state.data.get_mut(&key);

        if list.is_none() {
            return vec![];
        }

        if let DbValue::List(deque) = list.unwrap() {
            let mut result = Vec::new();

            for _ in 0..count {
                if let Some(v) = deque.pop_front() {
                    result.push(v);
                } else {
                    break;
                }
            }

            self.shared.notify.notify_one();
            result
        } else {
            vec![]
        }
    }

    /// Normalize negative indices for list operations
    fn normalize_index(idx: i64, len: i64) -> i64 {
        if idx < 0 {
            (len + idx).max(0)
        } else {
            idx.min(len - 1)
        }
    }
}

/// Background task to remove expired keys
async fn clean_expired(shared: Arc<Shared>) {
    loop {
        if let Some(next_expire) = purge(&shared) {
            tokio::select! {
                _ = tokio::time::sleep_until(next_expire) => {}
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
        let next = state
            .expirations
            .iter()
            .next()
            .map(|(&when, key)| (when, key.clone()))?;

        let (when, key) = next;

        if when > now {
            return Some(when);
        }

        state.data.remove(&key);
        state.expirations.remove(&when);
    }
}

impl Default for Db {
    fn default() -> Self {
        Self::new()
    }
}
