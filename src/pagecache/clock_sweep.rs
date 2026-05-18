// Copyright (c) 2026 黎展波 / Atlas Lee <4859345@qq.com>
// SPDX-License-Identifier: BSL-1.1
//
// Licensed under the Business Source License, version 1.1 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at:
//
//     https://mariadb.com/bsl11/
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//
//! Clock Sweep 缓存淘汰算法
//!
//! 教学说明：
//! - 每个缓存页有一个 "引用位"（ref bit）
//! - 扫描指针循环遍历所有页：
//!   - ref = 1 → 置为 0，跳过（给第二次机会）
//!   - ref = 0 → 淘汰该页
//! - 16 分区：将缓存分成 16 个独立子缓存，减少锁竞争
//!
//! 为什么选 Clock Sweep？
//! - 实现简单（一个循环指针 + 一个 bit）
//! - 近似 LRU 效果（Second Chance）
//! - 教学价值高（经典操作系统算法）

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::RwLock;

/// 缓存页
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

/// 缓存分区
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

    /// 获取页
    pub fn get(&self, key: u64) -> Option<Vec<u8>> {
        let pages = self.pages.read().unwrap();
        if let Some(page) = pages.iter().find(|p| p.key == key) {
            page.ref_bit.store(true, Ordering::Relaxed);
            return Some(page.data.clone());
        }
        None
    }

    /// 插入页
    pub fn insert(&self, key: u64, data: Vec<u8>) {
        let mut pages = self.pages.write().unwrap();

        // 如果已存在，更新
        if let Some(page) = pages.iter_mut().find(|p| p.key == key) {
            page.data = data;
            page.ref_bit.store(true, Ordering::Relaxed);
            return;
        }

        // 如果未满，直接插入
        if pages.len() < self.max_pages {
            pages.push(CachePage::new(key, data));
            return;
        }

        // Clock Sweep 找淘汰页
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

/// 页面缓存（多分区）
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
        let cache = PageCache::new(4, 1); // 单分区，4 页
        for i in 0..10 {
            cache.insert(i, vec![i as u8]);
        }
        // 早插入的可能被淘汰
        let mut hits = 0;
        for i in 0..10 {
            if cache.get(i).is_some() {
                hits += 1;
            }
        }
        assert!(hits <= 4); // 最多命中 4 个
    }

    #[test]
    fn test_clock_sweep_second_chance() {
        let shard = CacheShard::new(2);
        shard.insert(1, vec![1]);
        shard.insert(2, vec![2]);

        // 反复访问页 1，给它第二次机会
        for _ in 0..10 {
            shard.get(1);
        }

        // 插入新页，页 1 应该保留（因为被频繁访问）
        shard.insert(3, vec![3]);
        assert!(shard.get(1).is_some());
    }
}
