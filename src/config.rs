// Copyright 2026 Zhanbo Li / Atlas Lee <zhanbo.lee@gmail.com>
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

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::DaoQLError;

/// DaoQL-Edu global configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Data directory path
    pub data_dir: PathBuf,

    /// StorageEngine configuration
    pub storage: StorageConfig,

    /// WAL configuration
    pub wal: WalConfig,

    /// Page cache configuration
    pub page_cache: PageCacheConfig,

    /// Transaction configuration
    pub transaction: TransactionConfig,

    /// VectorEngine configuration
    pub vector: VectorConfig,

    /// HNSW configuration
    pub hnsw: HnswConfig,

    /// Column engine configuration
    pub column: ColumnConfig,

    /// Index persistence level (set to false in benchmarks to skip fsync)
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
    /// Load configuration from TOML file
    pub fn from_file(path: &PathBuf) -> Result<Self, DaoQLError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| DaoQLError::Path {
                path: path.clone(),
                reason: e.to_string(),
            })?;
        let config: Config = toml::from_str(&content)
            .map_err(|e| DaoQLError::Config(format!("TOML parse failed: {e}")))?;
        Ok(config)
    }

    /// Save as TOML file
    pub fn to_file(&self, path: &PathBuf) -> Result<(), DaoQLError> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| DaoQLError::Config(format!("TOML serialization failed: {e}")))?;
        std::fs::write(path, content)
            .map_err(|e| DaoQLError::Path {
                path: path.clone(),
                reason: e.to_string(),
            })?;
        Ok(())
    }

    /// Generate default configuration file content
    pub fn default_toml() -> String {
        let config = Config::default();
        toml::to_string_pretty(&config).unwrap_or_default()
    }
}

/// StorageEngine configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Initial node file size (record count)
    pub node_initial_capacity: usize,
    /// Initial edge file size (record count)
    pub edge_initial_capacity: usize,
    /// Growth factor (multiplier for each expansion)
    pub grow_factor: f64,
    /// Maximum node count
    pub max_nodes: usize,
    /// Maximum edge count
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

/// WAL configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalConfig {
    /// WAL file path (relative to data_dir)
    pub wal_dir: PathBuf,
    /// Buffer size (bytes)
    pub buffer_size: usize,
    /// Group commit timeout (milliseconds)
    pub flush_interval_ms: u64,
    /// Whether to sync flush (fsync each record)
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

/// Page cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageCacheConfig {
    /// Total page count
    pub total_pages: usize,
    /// Page size (bytes)
    pub page_size: usize,
    /// Partition count
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

/// Transaction configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionConfig {
    /// Lock acquisition timeout (milliseconds)
    pub lock_timeout_ms: u64,
    /// Whether to enable deadlock detection
    pub deadlock_detection: bool,
    /// Maximum active transaction count
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

/// VectorEngine configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorConfig {
    /// Default vector dimension
    pub default_dim: usize,
    /// Distance metric
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

/// HNSW configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HnswConfig {
    /// Max out-degree per layer
    pub m: usize,
    /// Search width during build
    pub ef_construction: usize,
    /// Search width during query
    pub ef_search: usize,
    /// Maximum element count
    pub max_elements: usize,
    /// Whether to enable quantization
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

/// Column engine configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnConfig {
    /// Granule size (bytes)
    pub granule_size: usize,
    /// Whether to enable compression
    pub enable_compression: bool,
    /// Compression level
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
