use std::{
    collections::HashMap,
    fmt,
    time::Duration,
};


use crate::Entry;

#[derive(Debug, PartialEq)]
pub enum StoreError {
    KeyNotFound(String),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::KeyNotFound(key) => write!(f, "key '{key}' not found"),
        }
    }
}

impl std::error::Error for StoreError {}

/// The in-memory database.
///
/// The underlying HashMap is private so callers must interact
/// through the Database API.
pub struct Database {
    data: HashMap<String, Entry>,
}

impl Database {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    /// Sets a key-value pair in the database.
    pub fn set(&mut self, key: &str, value: &str) {
        self.data
            .insert(key.to_string(), Entry::new(value.to_string()));
    }

    pub fn set_with_ttl(&mut self, key: &str, value: &str, ttl: Duration) {
        self.data
            .insert(key.to_string(), Entry::new_with_ttl(value.to_string(), ttl));
    }
    
    // Gets the value associated with a key in the database.
    pub fn get(&self, key: &str) -> Option<&str> {
        let entry = self.data.get(key)?;

        if entry.is_expired() {
            return None;
        }

        Some(entry.value.as_str())
    }

    /// Deletes a key-value pair from the database.
    pub fn delete(&mut self, key: &str) -> Option<String> {
        let entry = self.data.get(key)?;

        if entry.is_expired() {
            self.data.remove(key);
            return None;
        }

        self.data.remove(key).map(|entry| entry.value)
    }

    /// Checks for physically stored entry and has not expired
    pub fn exists(&self, key: &str) -> bool {
        match self.data.get(key) {
            Some(entry) => !entry.is_expired(),
            None => false,
        }
    }

    /// Returns an iterator over the keys in the database.
    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.data
            .iter()
            .filter(|(_, entry)| !entry.is_expired())
            .map(|(key, _)| key)
    }

    /// Returns the time-to-live (TTL) of a key in seconds. If the key does not exist, it returns -2.
    /// If the key exists but has no expiration, it returns -1. Otherwise, it returns the remaining TTL in seconds.
    pub fn ttl(&self, key: &str) -> i64 {
        let entry = match self.data.get(key) {
            Some(entry) => entry,
            None => return -2,
        };

        match entry.expires_at {
            None => -1,

            Some(expires_at) => {
                let now = std::time::Instant::now();

                if now >= expires_at {
                    -2
                } else {
                    expires_at.duration_since(now).as_secs() as i64
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::assert_eq;

    use super::*;

    #[test]
    fn creates_empty_database() {
        let db = Database::new();

        assert!(!db.exists("name"));
        assert_eq!(db.get("name"), None);
    }

    #[test]
    fn can_set_and_get_value() {
        let mut db = Database::new();

        db.set("name", "Alice");

        assert_eq!(db.get("name"), Some("Alice"));
    }

    #[test]
    fn can_delete_value() {
        let mut db = Database::new();

        db.set("name", "Alice");

        let deleted = db.delete("name");

        assert_eq!(deleted, Some("Alice".to_string()));
        assert!(!db.exists("name"));
    }

    #[test]
    fn can_iterate_over_keys() {
        let mut db = Database::new();

        db.set("name", "Alice");
        db.set("city", "Lagos");
        db.set("language", "Rust");

        let mut keys = db.keys();

        assert!(keys.any(|key| key == "name"));
    }

    #[test]
    fn returns_error_when_key_does_not_exist() {
        let db = Database::new();

        let result = db.get("missing");

        assert!(result.is_none());
    }

    #[test]
    fn returns_key_not_found_error() {
        let db = Database::new();
        let result = db.get("missing");

        match result {
            Some(value) => println!("found: {value}"),
            None => println!("key not found"),
        }
    }

    #[test]
    fn key_without_ttl_does_not_expire() {
        let mut db = Database::new();

        db.set("name", "Alice");

        assert_eq!(db.get("name"), Some("Alice"));
    }

    #[test]
    fn key_with_ttl_is_available_before_expiration() {
        let mut db = Database::new();

        db.set_with_ttl("session", "abc123", Duration::from_secs(60));
        assert_eq!(db.get("session"), Some("abc123"));
    }

    #[test]
    fn expired_key_is_treated_as_missing() {
        let mut db = Database::new();

        db.set_with_ttl("session", "abc123", Duration::from_millis(10));

        std::thread::sleep(Duration::from_millis(20));

        let result = db.get("session");

        assert_eq!(result, None);
    }

    #[test]
    fn expired_key_is_removed_from_database() {
        let mut db = Database::new();

        db.set_with_ttl("session", "abc123", Duration::from_millis(10));

        std::thread::sleep(Duration::from_millis(20));

        let _ = db.get("session");

        assert!(!db.exists("session"));
    }

    #[test]
    fn expired_keys_are_not_returned() {
        let mut db = Database::new();

        db.set("permanent", "value");

        db.set_with_ttl("temporary", "value", Duration::from_millis(10));

        std::thread::sleep(Duration::from_millis(20));

        let keys: Vec<&String> = db.keys().collect();

        assert!(keys.contains(&&"permanent".to_string()));
        assert!(!keys.iter().any(|key| key.as_str() == "temporary"));
    }

    #[test]
    fn ttl_returns_correct_values() {
        let mut db = Database::new();

        assert_eq!(db.ttl("missing"), -2);

        db.set("permanent", "value");
        assert_eq!(db.ttl("permanent"), -1);
    }

    #[test]
    fn ttl_returns_remaining_seconds() {
        let mut db = Database::new();

        db.set_with_ttl("session", "abc123", Duration::from_secs(60));

        let ttl = db.ttl("session");

        assert!(ttl > 0);
        assert!(ttl <= 60);
    }
}

// Sample for iterator lesson
// struct _FruitIterator<'a> {
//     fruits: &'a [String],
//     current: usize,
// }

// impl<'a> Iterator for _FruitIterator<'a> {
//     type Item = &'a String;

//     fn next(&mut self) -> Option<Self::Item> {
//         if self.current >= self.fruits.len() {
//             return None;
//         }

//         let fruit = &self.fruits[self.current];

//         self.current += 1;

//         Some(fruit)
//     }
// }
