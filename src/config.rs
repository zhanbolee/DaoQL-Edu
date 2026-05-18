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
//! 运行时配置管理
//!
//! 教学说明：
//! - 使用 TOML 格式，人类可读、易编辑
//! - 分层设计：默认值 → 配置文件 → 环境变量
//! - 教学版简化：不支持热重载

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::DaoQLError;

/// DaoQL-Edu 全局配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// 数据目录路径
    pub data_dir: PathBuf,

    /// 存储引擎配置
    pub storage: StorageConfig,

    /// WAL 配置
    pub wal: WalConfig,

    /// 页面缓存配置
    pub page_cache: PageCacheConfig,

    /// 事务配置
    pub transaction: TransactionConfig,

    /// 向量引擎配置
    pub vector: VectorConfig,

    /// HNSW 配置
    pub hnsw: HnswConfig,

    /// 列引擎配置
    pub column: ColumnConfig,

    /// 索引持久化级别（benchmark 可设为 false 跳过 fsync）
    pub index_sync: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("./data"),
            storage: StorageConfig::default(),
            wal: WalConfig::default(),
            page_cache: PageCacheConfig::default(),
            transaction: TransactionConfig::default(),
            vector: VectorConfig::default(),
            hnsw: HnswConfig::default(),
            column: ColumnConfig::default(),
            index_sync: true,
        }
    }
}

impl Config {
    /// 从 TOML 文件加载配置
    pub fn from_file(path: &PathBuf) -> Result<Self, DaoQLError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| DaoQLError::Path {
                path: path.clone(),
                reason: e.to_string(),
            })?;
        let config: Config = toml::from_str(&content)
            .map_err(|e| DaoQLError::Config(format!("TOML 解析失败: {e}")))?;
        Ok(config)
    }

    /// 保存为 TOML 文件
    pub fn to_file(&self, path: &PathBuf) -> Result<(), DaoQLError> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| DaoQLError::Config(format!("TOML 序列化失败: {e}")))?;
        std::fs::write(path, content)
            .map_err(|e| DaoQLError::Path {
                path: path.clone(),
                reason: e.to_string(),
            })?;
        Ok(())
    }

    /// 生成默认配置文件内容
    pub fn default_toml() -> String {
        let config = Config::default();
        toml::to_string_pretty(&config).unwrap_or_default()
    }
}

/// 存储引擎配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// 初始节点文件大小（记录数）
    pub node_initial_capacity: usize,
    /// 初始边文件大小（记录数）
    pub edge_initial_capacity: usize,
    /// 扩容因子（每次扩容的倍数）
    pub grow_factor: f64,
    /// 最大节点数
    pub max_nodes: usize,
    /// 最大边数
    pub max_edges: usize,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            node_initial_capacity: 1024,
            edge_initial_capacity: 4096,
            grow_factor: 2.0,
            max_nodes: 10_000_000,
            max_edges: 100_000_000,
        }
    }
}

/// WAL 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalConfig {
    /// WAL 文件路径（相对于 data_dir）
    pub wal_dir: PathBuf,
    /// 缓冲区大小（字节）
    pub buffer_size: usize,
    /// 组提交超时（毫秒）
    pub flush_interval_ms: u64,
    /// 是否同步刷盘（fsync 每个记录）
    pub sync_on_write: bool,
}

impl Default for WalConfig {
    fn default() -> Self {
        Self {
            wal_dir: PathBuf::from("wal"),
            buffer_size: 4 * 1024 * 1024, // 4MB
            flush_interval_ms: 10,
            sync_on_write: false,
        }
    }
}

/// 页面缓存配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageCacheConfig {
    /// 总页数
    pub total_pages: usize,
    /// 页大小（字节）
    pub page_size: usize,
    /// 分区数
    pub num_shards: usize,
}

impl Default for PageCacheConfig {
    fn default() -> Self {
        Self {
            total_pages: 4096,
            page_size: 64 * 1024, // 64KB
            num_shards: 16,
        }
    }
}

/// 事务配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionConfig {
    /// 锁获取超时（毫秒）
    pub lock_timeout_ms: u64,
    /// 是否启用死锁检测
    pub deadlock_detection: bool,
    /// 最大活跃事务数
    pub max_active_tx: usize,
}

impl Default for TransactionConfig {
    fn default() -> Self {
        Self {
            lock_timeout_ms: 5000,
            deadlock_detection: true,
            max_active_tx: 1024,
        }
    }
}

/// 向量引擎配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorConfig {
    /// 默认向量维度
    pub default_dim: usize,
    /// 距离度量方式
    pub distance: DistanceMetric,
}

impl Default for VectorConfig {
    fn default() -> Self {
        Self {
            default_dim: 384,
            distance: DistanceMetric::Cosine,
        }
    }
}

pub use crate::vector::distance::DistanceMetric;

/// HNSW 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HnswConfig {
    /// 每层最大出度
    pub m: usize,
    /// 构建时搜索宽度
    pub ef_construction: usize,
    /// 查询时搜索宽度
    pub ef_search: usize,
    /// 最大元素数
    pub max_elements: usize,
    /// 是否启用量化
    pub enable_quantization: bool,
}

impl Default for HnswConfig {
    fn default() -> Self {
        Self {
            m: 16,
            ef_construction: 100,
            ef_search: 64,
            max_elements: 100_000,
            enable_quantization: true,
        }
    }
}

/// 列引擎配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnConfig {
    /// Granule 大小（字节）
    pub granule_size: usize,
    /// 是否启用压缩
    pub enable_compression: bool,
    /// 压缩级别
    pub compression_level: u32,
}

impl Default for ColumnConfig {
    fn default() -> Self {
        Self {
            granule_size: 64 * 1024, // 64KB
            enable_compression: true,
            compression_level: 4,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let cfg = Config::default();
        assert_eq!(cfg.storage.node_initial_capacity, 1024);
        assert_eq!(cfg.wal.buffer_size, 4 * 1024 * 1024);
        assert_eq!(cfg.page_cache.total_pages, 4096);
    }

    #[test]
    fn test_config_roundtrip() {
        let cfg = Config::default();
        let toml_str = Config::default_toml();
        let cfg2: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(cfg.storage.node_initial_capacity, cfg2.storage.node_initial_capacity);
        assert_eq!(cfg.hnsw.m, cfg2.hnsw.m);
    }
}
