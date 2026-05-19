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

use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::error::DaoQLError;

pub mod file;
pub mod mmap;
pub mod page;

use mmap::MmapStore;

/// Storage manager
///
/// responsible for all data file init, open, close。
pub struct StorageManager {
    /// Data directory
    pub data_dir: PathBuf,
    /// Node mmap store
    pub nodes: MmapStore,
    /// Edge mmap store
    pub edges: MmapStore,
    /// Configuration
    pub config: Config,
}

impl StorageManager {
    /// OpenOrCreatestore
    pub fn open(data_dir: impl AsRef<Path>, config: Config) -> Result<Self, DaoQLError> {
        let data_dir = data_dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&data_dir)?;

        let nodes_path = data_dir.join("nodes.dat");
        let edges_path = data_dir.join("edges.dat");

        let nodes = MmapStore::open_or_create(
            &nodes_path,
            crate::graph::record::NodeRecord::SIZE,
            config.storage.node_initial_capacity,
        )?;

        let edges = MmapStore::open_or_create(
            &edges_path,
            crate::graph::record::EdgeRecord::SIZE,
            config.storage.edge_initial_capacity,
        )?;

        Ok(Self {
            data_dir,
            nodes,
            edges,
            config,
        })
    }

    /// Get columnstoredirectory
    pub fn column_dir(&self) -> PathBuf {
        self.data_dir.join("columns")
    }

    /// Getvectorstoredirectory
    pub fn vector_dir(&self) -> PathBuf {
        self.data_dir.join("vectors")
    }

    /// Get WAL directory
    pub fn wal_dir(&self) -> PathBuf {
        self.data_dir.join(&self.config.wal.wal_dir)
    }

    /// Getindexdirectory
    pub fn index_dir(&self) -> PathBuf {
        self.data_dir.join("index")
    }

    /// Closestore（Flush）
    pub fn shutdown(&mut self) -> Result<(), DaoQLError> {
        self.nodes.flush()?;
        self.edges.flush()?;
        Ok(())
    }

    /// AllocatenewNode record
    ///
    /// Return offset (bytes), caller uses this offset to access record
    pub fn alloc_node(&mut self) -> Result<u64, DaoQLError> {
        let offset = self.nodes.alloc()?;
        Ok(offset as u64 * crate::graph::record::NodeRecord::SIZE as u64)
    }

    /// AllocatenewEdge record
    pub fn alloc_edge(&mut self) -> Result<u64, DaoQLError> {
        let offset = self.edges.alloc()?;
        Ok(offset as u64 * crate::graph::record::EdgeRecord::SIZE as u64)
    }

    /// GetcurrentNode count
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// GetcurrentEdge count
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }
}
