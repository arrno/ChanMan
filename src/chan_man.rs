use rayon::prelude::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex}; // Import Rayon parallel iterators

type Callback = Arc<dyn Fn(&str) + Send + Sync>;

#[derive(Clone)]
pub struct ChanMan {
    subs: Arc<Mutex<HashMap<String, HashMap<String, Callback>>>>, // topic: { session: fn }
}

impl ChanMan {
    pub fn new() -> Self {
        Self {
            subs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn subscribe<F>(&self, session_key: String, topic: String, callback: F)
    where
        F: Fn(&str) + Send + Sync + 'static,
    {
        let mut subs = self.subs.lock().unwrap();
        subs.entry(topic)
            .or_insert_with(|| HashMap::new())
            .insert(session_key, Arc::new(callback));
    }

    pub fn unsubscribe(&self, session_key: String, topic: String) {
        let mut subs = self.subs.lock().unwrap();
        if let Some(sessions) = subs.get_mut(&topic) {
            sessions.remove(&session_key);
        }
    }

    pub fn publish(&self, topic: &str, message: &str) {
        let subs = self.subs.lock().unwrap();

        if let Some(subscribers) = subs.get(topic) {
            subscribers.par_iter().for_each(|(_, sub)| sub(message));
        }
    }
}
