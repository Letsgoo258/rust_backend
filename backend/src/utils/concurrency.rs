use std::hash::{Hash, Hasher};
use std::sync::Arc;
use tokio::sync::{Mutex, OwnedMutexGuard};

/// Number of partitioned lock stripes to minimize contention.
const NUM_STRIPES: usize = 128;

/// A high-performance stripped lock manager that provides key-level serialized execution
/// without blocking unrelated records or entire tables.
#[derive(Clone)]
pub struct KeyLockManager {
    stripes: Arc<Vec<Arc<Mutex<()>>>>,
}

impl Default for KeyLockManager {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyLockManager {
    pub fn new() -> Self {
        let mut stripes = Vec::with_capacity(NUM_STRIPES);
        for _ in 0..NUM_STRIPES {
            stripes.push(Arc::new(Mutex::new(())));
        }
        Self {
            stripes: Arc::new(stripes),
        }
    }

    /// Computes the stripe index for any hashable key.
    fn stripe_index<K: Hash>(&self, key: &K) -> usize {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut hasher);
        (hasher.finish() as usize) % NUM_STRIPES
    }

    /// Acquires an asynchronous lock guard specifically for the hashed key partition.
    pub async fn acquire_lock<K: Hash>(&self, key: &K) -> OwnedMutexGuard<()> {
        let idx = self.stripe_index(key);
        self.stripes[idx].clone().lock_owned().await
    }
}
