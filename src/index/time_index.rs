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

use std::path::Path;

use redb::{Database, ReadableTable, ReadableTableMetadata, TableDefinition};

use crate::error::DaoQLError;
use crate::id::BeingId;

/// redb table definition
/// Key = timestamp (millisecond Granule, used for reducing key quantity)
/// Value = postcard serialized BeingId list
const TIME_TABLE: TableDefinition<i64, &[u8]> = TableDefinition::new("time_index");

/// Time index
pub struct TimeIndex {
    db: Database,
    /// Time Granule: milliseconds (will group nanosecond timestamp by milliseconds)
    granularity_ms: i64,
}

impl TimeIndex {
    /// Open or create index database
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DaoQLError> {
        let db = Database::create(path)?;
        let tx = db.begin_write()?;
        {
            let _ = tx.open_table(TIME_TABLE)?;
        }
        tx.commit()?;
        Ok(Self {
            db,
            granularity_ms: 1000, // 1 second Granule
        })
    }

    /// will convert nanosecond timestamp to store key (milliseconds)
    fn key_of(&self, timestamp_ns: i64) -> i64 {
        timestamp_ns / 1_000_000 / self.granularity_ms * self.granularity_ms
    }

    /// Add Being to time index
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

    /// Range query: return all BeingId in [start_ns, end_ns)
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

        // Deduplicate
        result.sort();
        result.dedup();
        Ok(result)
    }

    /// Statisticsentry count（keyquantity）
    pub fn len(&self) -> Result<u64, DaoQLError> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TIME_TABLE)?;
        Ok(table.len()?)
    }

    /// Is empty
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
        let ts = 1_000_000_000_i64 * 1_000_000_000; // some nanosecond timestamp

        index.add(ts, id1).unwrap();
        index.add(ts + 500_000_000, id2).unwrap(); // within same second

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
            let ts = (i as i64) * 2_000_000_000; // every 2 seconds
            index.add(ts, *id).unwrap();
        }

        // Query 3rd to 6th (index 2 to 5)
        let start = 2 * 2_000_000_000;
        let end = 6 * 2_000_000_000;
        let results = index.range_query(start, end).unwrap();
        assert_eq!(results.len(), 4);
    }
}
