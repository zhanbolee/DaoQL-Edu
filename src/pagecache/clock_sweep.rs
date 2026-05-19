// Copyright (c) 2026 Zhanbo Li / Atlas Lee <4859345@qq.com>
// SPDX-License-Identifier: AGPL-3.0-or-later
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published
// by the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::RwLock;

/// Cachepage
pub struct CachePage {
    pub key: u64,
    pub data: Vec<u8>,
    pub ref_bit: AtomicBool,
    pub dirty: AtomicBool,
}

impl CachePage {
    pub fn new(key: u64, data: Vec<u8>) -> Self {
        Self {
            key,
            data,
            ref_bit: AtomicBool::new(false),
            dirty: AtomicBool::new(false),
        }
    }
}

/// Cachepartition
pub struct CacheShard {
    pub pages: RwLock<Vec<CachePage>>,
    pub clock_hand: AtomicUsize,
    pub max_pages: usize,
}

impl CacheShard {
    pub fn new(max_pages: usize) -> Self {
        Self {
            pages: RwLock::new(Vec::with_capacity(max_pages)),
            clock_hand: AtomicUsize::new(0),
            max_pages,
        }
    }

    /// Get page
    pub fn get(&self, key: u64) -> Option<Vec<u8>> {
        let pages = self.pages.read().unwrap();
        if let Some(page) = pages.iter().find(|p| p.key == key) {
            page.ref_bit.store(true, Ordering::Relaxed);
            return Some(page.data.clone());
        }
        None
    }

    /// insertpage
    pub fn insert(&self, key: u64, data: Vec<u8>) {
        let mut pages = self.pages.write().unwrap();

        // if already exists, update
        if let Some(page) = pages.iter_mut().find(|p| p.key == key) {
            page.data = data;
            page.ref_bit.store(true, Ordering::Relaxed);
            return;
        }

        // if not full, insert directly
        if pages.len() < self.max_pages {
            pages.push(CachePage::new(key, data));
            return;
        }

        // Clock sweep finds evictable page
        let start = self.clock_hand.load(Ordering::Relaxed);
        let mut victim = None;

        for i in 0..pages.len() {
            let idx = (start + i) % pages.len();
            let page = &pages[idx];
            if !page.ref_bit.swap(false, Ordering::Relaxed) {
                victim = Some(idx);
                break;
            }
        }

        let idx = victim.unwrap_or(start);
        pages[idx] = CachePage::new(key, data);
        self.clock_hand.store((idx + 1) % pages.len(), Ordering::Relaxed);
    }
}

/// Page cache (multi-partition)
pub struct PageCache {
    pub shards: Vec<CacheShard>,
    pub num_shards: usize,
}

impl PageCache {
    pub fn new(total_pages: usize, num_shards: usize) -> Self {
        let pages_per_shard = total_pages / num_shards;
        let shards = (0..num_shards)
            .map(|_| CacheShard::new(pages_per_shard))
            .collect();
        Self { shards, num_shards }
    }

    fn shard_idx(&self, key: u64) -> usize {
        (key as usize) % self.num_shards
    }

    pub fn get(&self, key: u64) -> Option<Vec<u8>> {
        self.shards[self.shard_idx(key)].get(key)
    }

    pub fn insert(&self, key: u64, data: Vec<u8>) {
        self.shards[self.shard_idx(key)].insert(key, data);
    }
}

impl Default for PageCache {
    fn default() -> Self {
        Self::new(4096, 16)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_basic() {
        let cache = PageCache::new(16, 4);
        cache.insert(1, vec![1, 2, 3]);
        assert_eq!(cache.get(1), Some(vec![1, 2, 3]));
        assert_eq!(cache.get(2), None);
    }

    #[test]
    fn test_cache_eviction() {
        let cache = PageCache::new(4, 1); // single partition, 4 pages
        for i in 0..10 {
            cache.insert(i, vec![i as u8]);
        }
        // early insert may be evicted
        let mut hits = 0;
        for i in 0..10 {
            if cache.get(i).is_some() {
                hits += 1;
            }
        }
        assert!(hits <= 4); // at most hit 4 
    }

    #[test]
    fn test_clock_sweep_second_chance() {
        let shard = CacheShard::new(2);
        shard.insert(1, vec![1]);
        shard.insert(2, vec![2]);

        // repeatedly access page 1, give it a second chance
        for _ in 0..10 {
            shard.get(1);
        }

        // insert new page, page 1 should be reserved (because frequently accessed)
        shard.insert(3, vec![3]);
        assert!(shard.get(1).is_some());
    }
}
