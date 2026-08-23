use std::time::{Duration, Instant};

#[derive(Debug, PartialEq)]
pub struct Entry {
    pub value: String,
    pub expires_at: Option<Instant>,
}

impl Entry {
    pub fn new(value: String) -> Self {
        Self {
            value,
            expires_at: None,
        }
    }

    pub fn new_with_ttl(value: String, ttl: Duration) -> Self {
        Self {
            value,
            expires_at: Some(Instant::now() + ttl),
        }
    }

    pub fn is_expired(&self) -> bool {
        match self.expires_at {
            Some(expires_at) => Instant::now() >= expires_at,
            None => false,
        }
    }
}
