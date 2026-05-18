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
//! 时间范围索引
//!
//! 教学说明：
//! - 使用 redb B+Tree，键 = 时间戳（i64，纳秒），值 = BeingId 列表（postcard 序列化）
//! - 支持 created_at/updated_at 范围查询
//! - 最终一致性：异步批量更新（事务不阻塞）

use std::path::Path;

use redb::{Database, ReadableTable, ReadableTableMetadata, TableDefinition};

use crate::error::DaoQLError;
use crate::id::BeingId;

/// redb 表定义
/// 键 = 时间戳（毫秒粒度，用于减少键数量）
/// 值 = postcard 序列化的 BeingId 列表
const TIME_TABLE: TableDefinition<i64, &[u8]> = TableDefinition::new("time_index");

/// 时间索引
pub struct TimeIndex {
    db: Database,
    /// 时间粒度：毫秒（将纳秒时间戳按毫秒分组存储）
    granularity_ms: i64,
}

impl TimeIndex {
    /// 打开或创建索引数据库
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DaoQLError> {
        let db = Database::create(path)?;
        let tx = db.begin_write()?;
        {
            let _ = tx.open_table(TIME_TABLE)?;
        }
        tx.commit()?;
        Ok(Self {
            db,
            granularity_ms: 1000, // 1 秒粒度
        })
    }

    /// 将纳秒时间戳转换为存储键（毫秒）
    fn key_of(&self, timestamp_ns: i64) -> i64 {
        timestamp_ns / 1_000_000 / self.granularity_ms * self.granularity_ms
    }

    /// 添加 Being 到时间索引
    pub fn add(&self, timestamp_ns: i64, id: BeingId) -> Result<(), DaoQLError> {
        let key = self.key_of(timestamp_ns);
        let tx = self.db.begin_write()?;
        {
            let mut table = tx.open_table(TIME_TABLE)?;
            let mut ids: Vec<BeingId> = match table.get(key)? {
                Some(v) => postcard::from_bytes(v.value()).unwrap_or_default(),
                None => Vec::new(),
            };
            if !ids.contains(&id) {
                ids.push(id);
                let bytes = postcard::to_allocvec(&ids)?;
                table.insert(key, bytes.as_slice())?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// 范围查询：返回 [start_ns, end_ns) 内的所有 BeingId
    pub fn range_query(
        &self,
        start_ns: i64,
        end_ns: i64,
    ) -> Result<Vec<BeingId>, DaoQLError> {
        let start_key = self.key_of(start_ns);
        let end_key = self.key_of(end_ns);

        let tx = self.db.begin_read()?;
        let table = tx.open_table(TIME_TABLE)?;

        let mut result = Vec::new();
        for item in table.range(start_key..end_key)? {
            let (_, value) = item?;
            let ids: Vec<BeingId> = postcard::from_bytes(value.value()).unwrap_or_default();
            result.extend(ids);
        }

        // 去重
        result.sort();
        result.dedup();
        Ok(result)
    }

    /// 统计条目数（键数量）
    pub fn len(&self) -> Result<u64, DaoQLError> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TIME_TABLE)?;
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
        let dir = std::env::temp_dir().join("daoql-edu-test-time-index");
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn test_time_index_basic() {
        let path = temp_path("test_basic.redb");
        let _ = std::fs::remove_file(&path);

        let index = TimeIndex::open(&path).unwrap();
        let id1 = BeingId::new();
        let id2 = BeingId::new();
        let ts = 1_000_000_000_i64 * 1_000_000_000; // 某个纳秒时间戳

        index.add(ts, id1).unwrap();
        index.add(ts + 500_000_000, id2).unwrap(); // 同一秒内

        let results = index.range_query(ts - 1, ts + 2_000_000_000).unwrap();
        assert!(results.contains(&id1));
        assert!(results.contains(&id2));
    }

    #[test]
    fn test_time_index_range_filtering() {
        let path = temp_path("test_range.redb");
        let _ = std::fs::remove_file(&path);

        let index = TimeIndex::open(&path).unwrap();
        let ids: Vec<_> = (0..10).map(|_| BeingId::new()).collect();

        for (i, id) in ids.iter().enumerate() {
            let ts = (i as i64) * 2_000_000_000; // 每 2 秒一个
            index.add(ts, *id).unwrap();
        }

        // 查询第 3 到第 6 个（索引 2 到 5）
        let start = 2 * 2_000_000_000;
        let end = 6 * 2_000_000_000;
        let results = index.range_query(start, end).unwrap();
        assert_eq!(results.len(), 4);
    }
}
