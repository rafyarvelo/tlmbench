//! The chunk cache: decoded chunks ([`tlmbench_core::chunks`]) of channels not stored as contiguous arrays, kept
//! in memory under a byte budget (25% of RAM by default) and evicted least recently used first. A chunk a query
//! holds is reference-counted: eviction skips it until it is released, so the budget can be passed briefly while
//! more is in use than it allows.
//!
//! Pyramid files and contiguous data (CSV cache columns, one-channel native `.tlm`) aren't kept here: they are read
//! through mmap and left to the OS page cache.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};

use tlmbench_core::chunks::Chunk;

/// A chunk's place: the opened recording (a number unique in the process), the channel and the chunk number.
pub(crate) type Key = (u64, usize, usize);

#[derive(Debug)]
pub struct ChunkCache {
    budget: u64,
    state: Mutex<State>,
}

#[derive(Debug, Default)]
struct State {
    bytes: u64,
    tick: u64,
    chunks: HashMap<Key, (Arc<Chunk>, u64)>,
    /// Last use → key, oldest first.
    order: BTreeMap<u64, Key>,
}

impl State {
    fn touch(&mut self, key: Key) -> Option<Arc<Chunk>> {
        self.tick += 1;
        let (chunk, used) = self.chunks.get_mut(&key)?;
        self.order.remove(used);
        *used = self.tick;
        self.order.insert(self.tick, key);
        Some(chunk.clone())
    }

    fn remove(&mut self, key: &Key) {
        if let Some((c, used)) = self.chunks.remove(key) {
            self.order.remove(&used);
            self.bytes -= c.bytes() as u64;
        }
    }
}

impl ChunkCache {
    /// A cache of at most `budget` bytes of chunks not in use.
    pub fn new(budget: u64) -> Self {
        Self { budget, state: Mutex::default() }
    }

    /// 25% of physical memory (1 GB when the OS doesn't say).
    pub fn default_budget() -> u64 {
        crate::sys::total_ram().map_or(1 << 30, |b| b / 4)
    }

    pub fn budget(&self) -> u64 {
        self.budget
    }

    /// Bytes of chunks held, in use or not.
    pub fn bytes(&self) -> u64 {
        self.state.lock().unwrap().bytes
    }

    /// Chunks held.
    pub fn len(&self) -> usize {
        self.state.lock().unwrap().chunks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The chunk at `key`, if cached (it becomes the most recently used).
    pub(crate) fn get(&self, key: Key) -> Option<Arc<Chunk>> {
        self.state.lock().unwrap().touch(key)
    }

    /// Caches `chunk` at `key` and returns it held; then evicts the least recently used chunks not in use until
    /// the cache is within its budget.
    pub(crate) fn insert(&self, key: Key, chunk: Chunk) -> Arc<Chunk> {
        let chunk = Arc::new(chunk);
        let mut s = self.state.lock().unwrap();
        s.remove(&key);
        s.tick += 1;
        let tick = s.tick;
        s.bytes += chunk.bytes() as u64;
        s.chunks.insert(key, (chunk.clone(), tick));
        s.order.insert(tick, key);
        if s.bytes > self.budget {
            let mut over = s.bytes - self.budget;
            let mut free = Vec::new();
            for k in s.order.values() {
                let c = &s.chunks[k].0;
                if Arc::strong_count(c) == 1 {
                    free.push(*k);
                    over = over.saturating_sub(c.bytes() as u64);
                    if over == 0 {
                        break;
                    }
                }
            }
            for k in free {
                s.remove(&k);
            }
        }
        chunk
    }

    /// Drops every chunk of opened recording `id` (when it closes); chunks in use are freed when released.
    pub(crate) fn forget(&self, id: u64) {
        let mut s = self.state.lock().unwrap();
        let keys: Vec<Key> = s.chunks.keys().filter(|k| k.0 == id).copied().collect();
        for k in keys {
            s.remove(&k);
        }
    }

    /// Drops every chunk not in use (benchmarks start cold with it).
    pub fn clear(&self) {
        let mut s = self.state.lock().unwrap();
        let keys: Vec<Key> = s.chunks.iter().filter(|(_, (c, _))| Arc::strong_count(c) == 1).map(|(k, _)| *k).collect();
        for k in keys {
            s.remove(&k);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tlmbench_core::chunks::CHUNK_SAMPLES;

    fn chunk() -> Chunk {
        Chunk::from_values(&vec![1.0f32; CHUNK_SAMPLES])
    }

    #[test]
    fn the_budget_holds_and_the_oldest_go_first() {
        let size = chunk().bytes() as u64;
        let cache = ChunkCache::new(10 * size);
        for k in 0..25 {
            drop(cache.insert((1, 0, k), chunk()));
            assert!(cache.bytes() <= cache.budget());
        }
        assert_eq!(cache.len(), 10);
        let _ = cache.get((1, 0, 15)); // used again: now the newest
        drop(cache.insert((1, 0, 99), chunk()));
        assert!(cache.get((1, 0, 15)).is_some() && cache.get((1, 0, 16)).is_none());
    }

    #[test]
    fn a_chunk_in_use_survives_eviction() {
        let size = chunk().bytes() as u64;
        let cache = ChunkCache::new(4 * size);
        let held = cache.insert((7, 3, 0), chunk());
        for k in 1..50 {
            drop(cache.insert((7, 3, k), chunk()));
        }
        assert!(Arc::ptr_eq(&cache.get((7, 3, 0)).expect("still cached"), &held));
        assert!(cache.bytes() <= cache.budget());
        // more in use than the budget allows: it is passed until they are released
        let all: Vec<_> = (100..110).map(|k| cache.insert((7, 3, k), chunk())).collect();
        assert_eq!(cache.len(), 11);
        drop(all);
        drop(held);
        drop(cache.insert((7, 3, 200), chunk()));
        assert!(cache.bytes() <= cache.budget());
        cache.forget(7);
        assert!(cache.is_empty());
    }
}
