// Copyright (c) 2026 Zhanbo Li / Atlas Lee <4859345@qq.com>
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
//! Storage Management Layer — unified entry for all data engines
//!
//! Educational Notes:
//! - StorageManager isdataenginelower layerInfrastructure
//! - provide mmap fixed-length record storage（NodeRecord/EdgeRecord）
//! - provide column file and vector file create/open
//! - unsafe code concentrated in mmap.rs, other modules remain safe

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
