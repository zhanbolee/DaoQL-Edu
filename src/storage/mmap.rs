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
//! mmap 存储抽象
//!
//! 教学说明：
//! - 这是项目中 unsafe 代码最集中的模块
//! - mmap 将文件映射到进程虚拟地址空间，实现零拷贝 I/O
//! - 定长记录设计：offset = index * record_size，O(1) 随机访问
//! - 自动扩容：当文件满时，重新 mmap 更大的文件
//!
//! SAFETY 注意事项：
//! - 所有指针操作前必须有长度检查
//! - mmap 生命周期绑定到 MmapStore（Drop 时自动 unmap）
//! - 多线程安全：通过 &mut self 保证排他访问

use std::fs::{File, OpenOptions};
use std::path::Path;

use memmap2::{MmapMut, MmapOptions};

use crate::error::{DaoQLError, StorageError};

/// mmap 定长记录存储
///
/// 将文件映射为定长记录的数组：
/// ```ignore
/// file_layout = [Record0][Record1][Record2]...[RecordN]
/// offset_of(i) = i * record_size
/// ```ignore
pub struct MmapStore {
    /// 底层文件
    file: File,
    /// mmap 映射
    mmap: MmapMut,
    /// 单条记录大小（字节）
    record_size: usize,
    /// 当前可容纳的记录数
    capacity: usize,
    /// 下一个可分配的索引
    next_free: usize,
    /// 文件路径（用于扩容时重新打开）
    #[allow(dead_code)]
    path: std::path::PathBuf,
    /// 初始容量（用于创建新文件）
    #[allow(dead_code)]
    initial_capacity: usize,
}

impl MmapStore {
    /// 打开或创建 mmap 存储
    ///
    /// # 参数
    /// - path: 文件路径
    /// - record_size: 单条记录大小（如 1536 或 256）
    /// - initial_capacity: 初始记录数（新文件时）
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
            // 已有文件：获取大小，计算容量
            let metadata = file.metadata()?;
            let file_size = metadata.len() as usize;
            let capacity = file_size / record_size;
            // SAFETY: 文件已存在且可写，mmap 生命周期由 self 管理
            let mmap = unsafe { MmapOptions::new().map_mut(&file)? };
            // 扫描找到 next_free（教学版简化：假设文件是紧凑的）
            let next_free = capacity; // 简化：假设所有位置都已使用
            (mmap, capacity, next_free)
        } else {
            // 新文件：预分配初始大小
            let file_size = record_size * initial_capacity;
            file.set_len(file_size as u64)?;
            // SAFETY: 新创建的文件，大小已设置
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

    /// 分配新的记录索引
    ///
    /// 返回索引号，调用者用 index * record_size 计算字节偏移
    pub fn alloc(&mut self) -> Result<usize, DaoQLError> {
        if self.next_free >= self.capacity {
            self.grow()?;
        }
        let idx = self.next_free;
        self.next_free += 1;
        Ok(idx)
    }

    /// 批量分配记录索引
    ///
    /// 相比逐条 alloc，只需一次 capacity 检查和可能的 grow，
    /// 减少锁/原子操作和 mmap 页错误。
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

    /// 扩容（翻倍）
    fn grow(&mut self) -> Result<(), DaoQLError> {
        let new_capacity = self.capacity * 2;
        let new_size = new_capacity * self.record_size;

        // 扩大文件
        self.file.set_len(new_size as u64)?;

        // 重新 mmap
        // SAFETY: 文件已扩大，旧 mmap 已 drop
        drop(std::mem::replace(
            &mut self.mmap,
            // SAFETY: 文件已扩大，我们有独占写权限
            unsafe { MmapOptions::new().len(new_size).map_mut(&self.file)? },
        ));

        self.capacity = new_capacity;
        Ok(())
    }

    /// 读取记录（只读）
    ///
    /// # SAFETY
    /// - index 必须在有效范围内
    /// - 返回的切片生命周期受 &self 约束
    pub fn get(&self, index: usize) -> Result<&[u8], DaoQLError> {
        if index >= self.capacity {
            return Err(DaoQLError::Storage(StorageError::OffsetOutOfBounds {
                offset: index,
                capacity: self.capacity,
            }));
        }
        let offset = index * self.record_size;
        // SAFETY: 已检查 index 范围，mmap 有效
        let slice = unsafe {
            std::slice::from_raw_parts(
                self.mmap.as_ptr().add(offset),
                self.record_size,
            )
        };
        Ok(slice)
    }

    /// 读取记录（可变）
    ///
    /// # SAFETY
    /// - index 必须在有效范围内
    /// - 返回的切片生命周期受 &mut self 约束，保证排他访问
    pub fn get_mut(&mut self, index: usize) -> Result<&mut [u8], DaoQLError> {
        if index >= self.capacity {
            return Err(DaoQLError::Storage(StorageError::OffsetOutOfBounds {
                offset: index,
                capacity: self.capacity,
            }));
        }
        let offset = index * self.record_size;
        // SAFETY: 已检查 index 范围，mmap 有效，&mut self 保证排他
        let slice = unsafe {
            std::slice::from_raw_parts_mut(
                self.mmap.as_mut_ptr().add(offset),
                self.record_size,
            )
        };
        Ok(slice)
    }

    /// 按字节偏移读取（用于外部索引存储的 offset）
    pub fn get_at_offset(&self, offset: u64) -> Result<&[u8], DaoQLError> {
        let index = offset as usize / self.record_size;
        self.get(index)
    }

    /// 按字节偏移可变读取
    pub fn get_mut_at_offset(&mut self, offset: u64) -> Result<&mut [u8], DaoQLError> {
        let index = offset as usize / self.record_size;
        self.get_mut(index)
    }

    /// 强制刷盘（fsync）
    pub fn flush(&mut self) -> Result<(), DaoQLError> {
        self.mmap.flush()?;
        Ok(())
    }

    /// 当前记录数
    pub fn len(&self) -> usize {
        self.next_free
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.next_free == 0
    }

    /// 当前容量
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// 记录大小
    pub fn record_size(&self) -> usize {
        self.record_size
    }

    /// 获取原始 mmap 指针（供高性能场景使用）
    ///
    /// # SAFETY
    /// - 调用者必须保证索引在有效范围内
    /// - 调用者必须保证没有数据竞争
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

        // 写入数据
        {
            let buf = store.get_mut(idx).unwrap();
            buf[0..4].copy_from_slice(b"TEST");
            buf[4..8].copy_from_slice(&42u32.to_le_bytes());
        }

        // 读取数据
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

        // 填满
        store.alloc().unwrap();
        store.alloc().unwrap();

        // 触发扩容
        store.alloc().unwrap();
        assert!(store.capacity() >= 4);
    }

    #[test]
    fn test_mmap_store_persistence() {
        let path = temp_path("test_persist.dat");
        let _ = std::fs::remove_file(&path);

        // 写入
        {
            let mut store = MmapStore::open_or_create(&path, 64, 4).unwrap();
            let idx = store.alloc().unwrap();
            let buf = store.get_mut(idx).unwrap();
            buf[0..4].copy_from_slice(b"KEEP");
            store.flush().unwrap();
        }

        // 重新打开
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
