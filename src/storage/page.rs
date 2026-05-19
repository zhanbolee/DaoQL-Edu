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
//! Page Allocator
//!
//! Educational Notes:
//! - page allocator manages free pages in mmap file
//! - use free list design：each free page stores next free page index
//! - allocate = take a page from linked list header
//! - release = put page back to linked list header
//! - time complexity：O(1) allocate/release

use crate::error::{DaoQLError, StorageError};

/// pageallocate
///
/// Manage fixed-size page free allocation。
/// free page list stored in mmap first page（page 0 as metadata page）。
#[derive(Debug)]
pub struct PageAllocator {
    /// Page size (bytes)
    page_size: usize,
    /// Total page count
    total_pages: usize,
    /// free page list head（0 representno free page）
    free_list_head: u32,
    /// Already allocated page count
    allocated: usize,
}

impl PageAllocator {
    /// Createnewpageallocate
    pub fn new(page_size: usize, total_pages: usize) -> Self {
        Self {
            page_size,
            total_pages,
            free_list_head: 1, // page 0 as metadata page，start from page 1
            allocated: 0,
        }
    }

    /// Allocateone page，returnpage index
    pub fn alloc(&mut self) -> Result<u32, DaoQLError> {
        if self.free_list_head == 0 || self.free_list_head as usize >= self.total_pages {
            return Err(DaoQLError::Storage(StorageError::CapacityExceeded {
                need: 1,
                remaining: self.total_pages - self.allocated,
            }));
        }
        let page_idx = self.free_list_head;
        self.free_list_head += 1; // simplify: sequential allocate
        self.allocated += 1;
        Ok(page_idx)
    }

    /// Releaseone page
    pub fn free(&mut self, page_idx: u32) -> Result<(), DaoQLError> {
        if page_idx == 0 || page_idx as usize >= self.total_pages {
            return Err(DaoQLError::Storage(StorageError::OffsetOutOfBounds {
                offset: page_idx as usize,
                capacity: self.total_pages,
            }));
        }
        // simplified version: don't maintain real free list, just reduce count
        // production version should maintain doubly linked list
        self.allocated -= 1;
        Ok(())
    }

    /// page index → byte offset
    pub fn offset_of(&self, page_idx: u32) -> u64 {
        page_idx as u64 * self.page_size as u64
    }

    /// byte offset → page index
    pub fn page_of(&self, offset: u64) -> u32 {
        (offset / self.page_size as u64) as u32
    }

    /// Already allocated page count
    pub fn allocated(&self) -> usize {
        self.allocated
    }

    /// free page count
    pub fn free_count(&self) -> usize {
        self.total_pages - self.allocated
    }

    /// utilization rate
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
        assert!(pa.alloc().is_err()); // 3 is boundary, simplified version doesn't allocate
    }

    #[test]
    fn test_page_offset_conversion() {
        let pa = PageAllocator::new(4096, 100);
        assert_eq!(pa.offset_of(5), 5 * 4096);
        assert_eq!(pa.page_of(5 * 4096), 5);
    }
}
