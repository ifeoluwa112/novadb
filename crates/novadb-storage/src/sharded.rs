use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use tokio::sync::RwLock;

use crate::Database;

/// A key-value store split into independent shards, each with its own lock.
/// A command touching key "alice" and a command touching key "zebra" can run
/// fully in parallel if they land in different shards — they never contend.
pub struct ShardedDatabase {
    shards: Vec<RwLock<Database>>,
}

impl ShardedDatabase {
    pub fn new(shard_count: usize) -> Self {
        assert!(shard_count > 0, "must have at least one shard");

        let shards = (0..shard_count).map(|_| RwLock::new(Database::new())).collect();

        Self { shards }
    }

    pub fn shard_count(&self) -> usize {
        self.shards.len()
    }

    /// The doorman: always sends the same key to the same shard.
    fn shard_index(&self, key: &str) -> usize {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        (hasher.finish() as usize) % self.shards.len()
    }

    /// The lock guarding the shard this key belongs to.
    pub fn shard_for(&self, key: &str) -> &RwLock<Database> {
        &self.shards[self.shard_index(key)]
    }

    /// For commands like KEYS that have no single key to route by.
    pub fn all_shards(&self) -> &[RwLock<Database>] {
        &self.shards
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_key_always_routes_to_same_shard() {
        let db = ShardedDatabase::new(16);
        let first = db.shard_index("alice");
        let second = db.shard_index("alice");
        assert_eq!(first, second);
    }

    #[test]
    fn different_keys_can_land_on_different_shards() {
        let db = ShardedDatabase::new(16);
        let indices: std::collections::HashSet<usize> = (0..100)
            .map(|i| db.shard_index(&format!("key-{i}")))
            .collect();

        // Not a strict guarantee, but with 100 keys over 16 shards we should
        // see meaningfully more than 1 distinct shard used.
        assert!(indices.len() > 1);
    }
}