// Copyright 2026 Zhanbo Li / Atlas Lee <4859345@qq.com>
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::fs::{File, OpenOptions};
use std::path::Path;

use memmap2::{MmapMut, MmapOptions};

use crate::error::{DaoQLError, StorageError};

/// mmap fixed-length record storage
///
/// will file map as fixed-length record count group：
/// ```ignore
/// file_layout = [Record0][Record1][Record2]...[RecordN]
/// offset_of(i) = i * record_size
/// ```ignore
pub struct MmapStore {
    /// Lower layerfile
    file: File,
    /// mmap map
    mmap: MmapMut,
    /// single record size（bytes）
    record_size: usize,
    /// Current capacity record count
    capacity: usize,
    /// Next allocatable index
    next_free: usize,
    /// File path (used for re-open when expanding)
    #[allow(dead_code)]
    path: std::path::PathBuf,
    /// Initial capacity (used for creating new file)
    #[allow(dead_code)]
    initial_capacity: usize,
}

impl MmapStore {
    /// OpenOrCreate mmap store
    ///
    /// # parameter
    /// - path: filepath
    /// - record_size: single record size（e.g. 1536 Or 256）
    /// - initial_capacity: initial record count（when new file）
    pub fn open_or_create(
        path: impl AsRef<Path>,
        record_size: usize,
        initial_capacity: usize,
    ) -> Result<Self, DaoQLError> {
        let path = path.as_ref().to_path_buf();

        let file_exists = path.exists();
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(|e| DaoQLError::Storage(StorageError::FileOpen {
                path: path.clone(),
                source: e,
            }))?;

        let (mmap, capacity, next_free) = if file_exists {
            // already has file: get size, compute capacity
            let metadata = file.metadata()?;
            let file_size = metadata.len() as usize;
            let capacity = file_size / record_size;
            // SAFETY: file already exists and writable，mmap lifecycle managed by self
            let mmap = unsafe { MmapOptions::new().map_mut(&file)? };
            // Scan to find next_free (edu edition simplification: assume file is compact)
            let next_free = capacity; // simplify: assume all positions already used
            (mmap, capacity, next_free)
        } else {
            // new file: pre-allocate initial size
            let file_size = record_size * initial_capacity;
            file.set_len(file_size as u64)?;
            // SAFETY: newCreatefile，sizealreadyset
            let mmap = unsafe { MmapOptions::new().len(file_size).map_mut(&file)? };
            (mmap, initial_capacity, 0)
        };

        Ok(Self {
            file,
            mmap,
            record_size,
            capacity,
            next_free,
            path,
            initial_capacity,
        })
    }

    /// Allocatenewrecordindex
    ///
    /// Return index number, caller uses index * record_size to compute byte offset
    pub fn alloc(&mut self) -> Result<usize, DaoQLError> {
        if self.next_free >= self.capacity {
            self.grow()?;
        }
        let idx = self.next_free;
        self.next_free += 1;
        Ok(idx)
    }

    /// Batchallocaterecordindex
    ///
    /// compared to individual alloc, only one capacity check and possible grow，
    /// reduce lock/atomic operations and mmap page errors。
    pub fn alloc_batch(&mut self, count: usize) -> Result<Vec<usize>, DaoQLError> {
        if count == 0 {
            return Ok(Vec::new());
        }
        while self.next_free + count > self.capacity {
            self.grow()?;
        }
        let start = self.next_free;
        self.next_free += count;
        Ok((start..start + count).collect())
    }

    /// expand (double)
    fn grow(&mut self) -> Result<(), DaoQLError> {
        let new_capacity = self.capacity * 2;
        let new_size = new_capacity * self.record_size;

        // expand file
        self.file.set_len(new_size as u64)?;

        // re- mmap
        // SAFETY: file expanded，old mmap already drop
        drop(std::mem::replace(
            &mut self.mmap,
            // SAFETY: file expanded, we have exclusive write permission
            unsafe { MmapOptions::new().len(new_size).map_mut(&self.file)? },
        ));

        self.capacity = new_capacity;
        Ok(())
    }

    /// Readrecord（read-only）
    ///
    /// # SAFETY
    /// - index must be in valid range
    /// - returned slice lifetime is bound by &self constraint
    pub fn get(&self, index: usize) -> Result<&[u8], DaoQLError> {
        if index >= self.capacity {
            return Err(DaoQLError::Storage(StorageError::OffsetOutOfBounds {
                offset: index,
                capacity: self.capacity,
            }));
        }
        let offset = index * self.record_size;
        // SAFETY: checked index range，mmap valid
        let slice = unsafe {
            std::slice::from_raw_parts(
                self.mmap.as_ptr().add(offset),
                self.record_size,
            )
        };
        Ok(slice)
    }

    /// Readrecord（mutable）
    ///
    /// # SAFETY
    /// - index must be in valid range
    /// - returned slice lifetime is bound by &mut self constraint，guarantee exclusive access
    pub fn get_mut(&mut self, index: usize) -> Result<&mut [u8], DaoQLError> {
        if index >= self.capacity {
            return Err(DaoQLError::Storage(StorageError::OffsetOutOfBounds {
                offset: index,
                capacity: self.capacity,
            }));
        }
        let offset = index * self.record_size;
        // SAFETY: checked index range，mmap valid，&mut self guarantee exclusive
        let slice = unsafe {
            std::slice::from_raw_parts_mut(
                self.mmap.as_mut_ptr().add(offset),
                self.record_size,
            )
        };
        Ok(slice)
    }

    /// Bybyte offsetread（used forexternalindexstore offset）
    pub fn get_at_offset(&self, offset: u64) -> Result<&[u8], DaoQLError> {
        let index = offset as usize / self.record_size;
        self.get(index)
    }

    /// Bybyte offsetmutableread
    pub fn get_mut_at_offset(&mut self, offset: u64) -> Result<&mut [u8], DaoQLError> {
        let index = offset as usize / self.record_size;
        self.get_mut(index)
    }

    /// forceFlush（fsync）
    pub fn flush(&mut self) -> Result<(), DaoQLError> {
        self.mmap.flush()?;
        Ok(())
    }

    /// Currentrecord count
    pub fn len(&self) -> usize {
        self.next_free
    }

    /// Is empty
    pub fn is_empty(&self) -> bool {
        self.next_free == 0
    }

    /// Current capacity
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Recordsize
    pub fn record_size(&self) -> usize {
        self.record_size
    }

    /// Get raw mmap pointer (for high-performance use)
    ///
    /// # SAFETY
    /// - caller must guarantee index in valid range
    /// - caller must guarantee no data race
    pub unsafe fn raw_ptr(&mut self) -> *mut u8 {
        self.mmap.as_mut_ptr()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("daoql-edu-test");
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn test_mmap_store_create_and_alloc() {
        let path = temp_path("test_create.dat");
        let _ = std::fs::remove_file(&path);

        let mut store = MmapStore::open_or_create(&path, crate::graph::record::EdgeRecord::SIZE, 4).unwrap();
        assert_eq!(store.capacity(), 4);
        assert_eq!(store.len(), 0);

        let idx = store.alloc().unwrap();
        assert_eq!(idx, 0);
        assert_eq!(store.len(), 1);

        let idx2 = store.alloc().unwrap();
        assert_eq!(idx2, 1);
    }

    #[test]
    fn test_mmap_store_read_write() {
        let path = temp_path("test_rw.dat");
        let _ = std::fs::remove_file(&path);

        let mut store = MmapStore::open_or_create(&path, 64, 4).unwrap();
        let idx = store.alloc().unwrap();

        // Writedata
        {
            let buf = store.get_mut(idx).unwrap();
            buf[0..4].copy_from_slice(b"TEST");
            buf[4..8].copy_from_slice(&42u32.to_le_bytes());
        }

        // Read data
        {
            let buf = store.get(idx).unwrap();
            assert_eq!(&buf[0..4], b"TEST");
            let val = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);
            assert_eq!(val, 42);
        }
    }

    #[test]
    fn test_mmap_store_grow() {
        let path = temp_path("test_grow.dat");
        let _ = std::fs::remove_file(&path);

        let mut store = MmapStore::open_or_create(&path, 64, 2).unwrap();
        assert_eq!(store.capacity(), 2);

        // fill up
        store.alloc().unwrap();
        store.alloc().unwrap();

        // trigger expansion
        store.alloc().unwrap();
        assert!(store.capacity() >= 4);
    }

    #[test]
    fn test_mmap_store_persistence() {
        let path = temp_path("test_persist.dat");
        let _ = std::fs::remove_file(&path);

        // Write
        {
            let mut store = MmapStore::open_or_create(&path, 64, 4).unwrap();
            let idx = store.alloc().unwrap();
            let buf = store.get_mut(idx).unwrap();
            buf[0..4].copy_from_slice(b"KEEP");
            store.flush().unwrap();
        }

        // re-open
        {
            let store = MmapStore::open_or_create(&path, 64, 4).unwrap();
            let buf = store.get(0).unwrap();
            assert_eq!(&buf[0..4], b"KEEP");
        }
    }

    #[test]
    fn test_mmap_store_out_of_bounds() {
        let path = temp_path("test_oob.dat");
        let _ = std::fs::remove_file(&path);

        let store = MmapStore::open_or_create(&path, 64, 4).unwrap();
        assert!(store.get(100).is_err());
    }
}
