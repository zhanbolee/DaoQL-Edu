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
//! 存储管理层 — 所有数据引擎的统一入口
//!
//! 教学说明：
//! - StorageManager 是数据引擎的底层基础设施
//! - 提供 mmap 定长记录存储（NodeRecord/EdgeRecord）
//! - 提供列文件和向量文件的创建/打开
//! - unsafe 代码集中在 mmap.rs 中，其余模块保持 safe

use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::error::DaoQLError;

pub mod file;
pub mod mmap;
pub mod page;

use mmap::MmapStore;

/// 存储管理器
///
/// 负责所有数据文件的初始化、打开、关闭。
pub struct StorageManager {
    /// 数据目录
    pub data_dir: PathBuf,
    /// 节点 mmap 存储
    pub nodes: MmapStore,
    /// 边 mmap 存储
    pub edges: MmapStore,
    /// 配置
    pub config: Config,
}

impl StorageManager {
    /// 打开或创建存储
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

    /// 获取列存储目录
    pub fn column_dir(&self) -> PathBuf {
        self.data_dir.join("columns")
    }

    /// 获取向量存储目录
    pub fn vector_dir(&self) -> PathBuf {
        self.data_dir.join("vectors")
    }

    /// 获取 WAL 目录
    pub fn wal_dir(&self) -> PathBuf {
        self.data_dir.join(&self.config.wal.wal_dir)
    }

    /// 获取索引目录
    pub fn index_dir(&self) -> PathBuf {
        self.data_dir.join("index")
    }

    /// 关闭存储（刷盘）
    pub fn shutdown(&mut self) -> Result<(), DaoQLError> {
        self.nodes.flush()?;
        self.edges.flush()?;
        Ok(())
    }

    /// 分配新的节点记录
    ///
    /// 返回偏移量（字节），调用者用此偏移访问记录
    pub fn alloc_node(&mut self) -> Result<u64, DaoQLError> {
        let offset = self.nodes.alloc()?;
        Ok(offset as u64 * crate::graph::record::NodeRecord::SIZE as u64)
    }

    /// 分配新的边记录
    pub fn alloc_edge(&mut self) -> Result<u64, DaoQLError> {
        let offset = self.edges.alloc()?;
        Ok(offset as u64 * crate::graph::record::EdgeRecord::SIZE as u64)
    }

    /// 获取当前节点数量
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// 获取当前边数量
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }
}
