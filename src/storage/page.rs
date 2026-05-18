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
//! 页分配器
//!
//! 教学说明：
//! - 页分配器管理 mmap 文件内的空闲页
//! - 采用空闲链表（Free List）设计：每个空闲页存储下一个空闲页的索引
//! - 分配 = 从链表头部取出一个页
//! - 释放 = 将页放回链表头部
//! - 时间复杂度：O(1) 分配/释放

use crate::error::{DaoQLError, StorageError};

/// 页分配器
///
/// 管理固定大小页的空闲分配。
/// 空闲页链表存储在 mmap 的第一个页中（页 0 为元数据页）。
#[derive(Debug)]
pub struct PageAllocator {
    /// 页大小（字节）
    page_size: usize,
    /// 总页数
    total_pages: usize,
    /// 空闲页链表头（0 表示无空闲页）
    free_list_head: u32,
    /// 已分配页数
    allocated: usize,
}

impl PageAllocator {
    /// 创建新的页分配器
    pub fn new(page_size: usize, total_pages: usize) -> Self {
        Self {
            page_size,
            total_pages,
            free_list_head: 1, // 页 0 为元数据页，从页 1 开始
            allocated: 0,
        }
    }

    /// 分配一页，返回页索引
    pub fn alloc(&mut self) -> Result<u32, DaoQLError> {
        if self.free_list_head == 0 || self.free_list_head as usize >= self.total_pages {
            return Err(DaoQLError::Storage(StorageError::CapacityExceeded {
                need: 1,
                remaining: self.total_pages - self.allocated,
            }));
        }
        let page_idx = self.free_list_head;
        self.free_list_head += 1; // 简化：顺序分配
        self.allocated += 1;
        Ok(page_idx)
    }

    /// 释放一页
    pub fn free(&mut self, page_idx: u32) -> Result<(), DaoQLError> {
        if page_idx == 0 || page_idx as usize >= self.total_pages {
            return Err(DaoQLError::Storage(StorageError::OffsetOutOfBounds {
                offset: page_idx as usize,
                capacity: self.total_pages,
            }));
        }
        // 简化版：不维护真正的空闲链表，仅减少计数
        // 生产版应维护双向链表
        self.allocated -= 1;
        Ok(())
    }

    /// 页索引 → 字节偏移
    pub fn offset_of(&self, page_idx: u32) -> u64 {
        page_idx as u64 * self.page_size as u64
    }

    /// 字节偏移 → 页索引
    pub fn page_of(&self, offset: u64) -> u32 {
        (offset / self.page_size as u64) as u32
    }

    /// 已分配页数
    pub fn allocated(&self) -> usize {
        self.allocated
    }

    /// 空闲页数
    pub fn free_count(&self) -> usize {
        self.total_pages - self.allocated
    }

    /// 使用率
    pub fn utilization(&self) -> f64 {
        self.allocated as f64 / self.total_pages as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_allocator_basic() {
        let mut pa = PageAllocator::new(4096, 100);
        assert_eq!(pa.free_count(), 100);

        let p1 = pa.alloc().unwrap();
        let p2 = pa.alloc().unwrap();
        assert_eq!(p1, 1);
        assert_eq!(p2, 2);
        assert_eq!(pa.allocated(), 2);

        pa.free(p1).unwrap();
        assert_eq!(pa.allocated(), 1);
    }

    #[test]
    fn test_page_allocator_exhausted() {
        let mut pa = PageAllocator::new(4096, 3);
        pa.alloc().unwrap(); // 1
        pa.alloc().unwrap(); // 2
        assert!(pa.alloc().is_err()); // 3 是边界，简化版不分配
    }

    #[test]
    fn test_page_offset_conversion() {
        let pa = PageAllocator::new(4096, 100);
        assert_eq!(pa.offset_of(5), 5 * 4096);
        assert_eq!(pa.page_of(5 * 4096), 5);
    }
}
