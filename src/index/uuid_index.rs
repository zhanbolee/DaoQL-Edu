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
//! UUID → NodeOffset 索引
//!
//! 教学说明：
//! - 使用 redb B+Tree，键 = UUID 字符串，值 = NodeOffset（u64）
//! - 强一致性：写入事务完成时同步更新
//! - 点查路径：UUID → redb 查 offset → mmap 读 NodeRecord

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;

use redb::{Database, Durability, ReadableTableMetadata, TableDefinition};

use crate::error::DaoQLError;
use crate::id::{BeingId, NodeOffset};

/// redb 表定义
const UUID_TABLE: TableDefinition<&str, u64> = TableDefinition::new("uuid_to_offset");

/// UUID 索引
pub struct UuidIndex {
    db: Database,
    /// 事务持久化级别（benchmark 可设为 None 跳过 fsync）
    durability: Durability,
    /// 内存缓存（benchmark 模式启用，避免 redb B-tree 遍历）
    cache: RefCell<Option<HashMap<String, NodeOffset>>>,
}

impl UuidIndex {
    /// 打开或创建索引数据库
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DaoQLError> {
        let db = Database::create(path)?;
        // 初始化表
        let tx = db.begin_write()?;
        {
            let _ = tx.open_table(UUID_TABLE)?;
        }
        tx.commit()?;
        Ok(Self {
            db,
            durability: Durability::Immediate,
            cache: RefCell::new(None),
        })
    }

    /// 设置持久化级别（用于 benchmark 模式跳过 fsync）
    pub fn set_durability(&mut self, durability: Durability) {
        self.durability = durability;
    }

    /// 启用内存缓存（benchmark 模式大幅加速点查）
    pub fn enable_cache(&self) {
        *self.cache.borrow_mut() = Some(HashMap::new());
    }

    /// 插入映射
    pub fn insert(&self, id: BeingId, offset: NodeOffset) -> Result<(), DaoQLError> {
        let mut tx = self.db.begin_write()?;
        tx.set_durability(self.durability);
        {
            let mut table = tx.open_table(UUID_TABLE)?;
            table.insert(id.to_hex().as_str(), offset)?;
        }
        tx.commit()?;
        if let Some(ref mut cache) = *self.cache.borrow_mut() {
            cache.insert(id.to_hex(), offset);
        }
        Ok(())
    }

    /// 查询偏移
    pub fn get(&self, id: BeingId) -> Result<Option<NodeOffset>, DaoQLError> {
        let hex = id.to_hex();
        if let Some(ref cache) = *self.cache.borrow() {
            return Ok(cache.get(&hex).copied());
        }
        let tx = self.db.begin_read()?;
        let table = tx.open_table(UUID_TABLE)?;
        match table.get(hex.as_str())? {
            Some(v) => Ok(Some(v.value())),
            None => Ok(None),
        }
    }

    /// 删除映射
    pub fn remove(&self, id: BeingId) -> Result<bool, DaoQLError> {
        let mut tx = self.db.begin_write()?;
        tx.set_durability(self.durability);
        let existed = {
            let mut table = tx.open_table(UUID_TABLE)?;
            let x = table.remove(id.to_hex().as_str())?.is_some();
            x
        };
        tx.commit()?;
        if let Some(ref mut cache) = *self.cache.borrow_mut() {
            cache.remove(&id.to_hex());
        }
        Ok(existed)
    }

    /// 批量插入（同一事务）
    pub fn batch_insert(&self, items: &[(BeingId, NodeOffset)]) -> Result<(), DaoQLError> {
        let mut tx = self.db.begin_write()?;
        tx.set_durability(self.durability);
        {
            let mut table = tx.open_table(UUID_TABLE)?;
            for (id, offset) in items {
                table.insert(id.to_hex().as_str(), *offset)?;
            }
        }
        tx.commit()?;
        if let Some(ref mut cache) = *self.cache.borrow_mut() {
            for (id, offset) in items {
                cache.insert(id.to_hex(), *offset);
            }
        }
        Ok(())
    }

    /// 统计条目数
    pub fn len(&self) -> Result<u64, DaoQLError> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(UUID_TABLE)?;
        Ok(table.len()?)
    }

    /// 是否为空
    pub fn is_empty(&self) -> Result<bool, DaoQLError> {
        Ok(self.len()? == 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("daoql-edu-test-uuid-index");
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn test_uuid_index_basic() {
        let path = temp_path("test_basic.redb");
        let _ = std::fs::remove_file(&path);

        let index = UuidIndex::open(&path).unwrap();
        let id = BeingId::new();
        let offset = 12345u64;

        assert!(index.get(id).unwrap().is_none());

        index.insert(id, offset).unwrap();
        assert_eq!(index.get(id).unwrap(), Some(offset));

        index.remove(id).unwrap();
        assert!(index.get(id).unwrap().is_none());
    }

    #[test]
    fn test_uuid_index_batch() {
        let path = temp_path("test_batch.redb");
        let _ = std::fs::remove_file(&path);

        let index = UuidIndex::open(&path).unwrap();
        let items: Vec<_> = (0..100)
            .map(|i| (BeingId::new(), i as u64 * crate::graph::record::NodeRecord::SIZE as u64))
            .collect();

        index.batch_insert(&items).unwrap();
        assert_eq!(index.len().unwrap(), 100);

        for (id, offset) in &items {
            assert_eq!(index.get(*id).unwrap(), Some(*offset));
        }
    }
}
