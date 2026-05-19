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

use crate::id::BeingId;
use crate::transaction::TxId;

/// DaoQL-Edu unified error type
#[derive(thiserror::Error, Debug)]
pub enum DaoQLError {
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),

    #[error("Index error: {0}")]
    Index(#[from] IndexError),

    #[error("Transaction error: {0}")]
    Transaction(#[from] TransactionError),

    #[error("Query error: {0}")]
    Query(#[from] QueryError),

    #[error("DSL parse error: {0}")]
    DslParse(String),

    #[error("Config error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("redb database error: {0}")]
    RedbDatabase(String),

    #[error("redb Transaction error: {0}")]
    RedbTransaction(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] postcard::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Not Found: Being {0:?}")]
    NotFound(BeingId),

    #[error("Type mismatch: expected {expected}, actual {actual}")]
    TypeMismatch { expected: String, actual: String },

    #[error("Constraint violation: {0}")]
    ConstraintViolation(String),

    #[error("MVCC conflict: transaction {tx_id} read uncommitted data")]
    MvccConflict { tx_id: TxId },

    #[error("WAL error: {0}")]
    Wal(String),

    #[error("Invalid state: {0}")]
    InvalidState(String),

    #[error("Unsupported feature: {0}")]
    Unsupported(String),

    #[error("Insufficient resources: {0}")]
    ResourceExhausted(String),

    #[error("Path error: {path} — {reason}")]
    Path { path: PathBuf, reason: String },
}

/// Storage-related errors
#[derive(thiserror::Error, Debug)]
pub enum StorageError {
    #[error("mmap failed: {0}")]
    Mmap(String),

    #[error("File open failed: {path} — {source}")]
    FileOpen { path: PathBuf, source: std::io::Error },

    #[error("Record size mismatch: expected {expected}, actual {actual}")]
    RecordSizeMismatch { expected: usize, actual: usize },

    #[error("Offset out of bounds: offset={offset}, capacity={capacity}")]
    OffsetOutOfBounds { offset: usize, capacity: usize },

    #[error("Insufficient capacity: need {need}, remaining {remaining}")]
    CapacityExceeded { need: usize, remaining: usize },

    #[error("Validation failed: {0}")]
    Corruption(String),
}

/// Index-related errors
#[derive(thiserror::Error, Debug)]
pub enum IndexError {
    #[error("redb error: {0}")]
    Redb(#[from] redb::Error),

    #[error("Key not found: {0}")]
    KeyNotFound(String),

    #[error("Duplicate key: {0}")]
    DuplicateKey(String),

    #[error("Range query error: {0}")]
    RangeQuery(String),
}

/// Transaction-related errors
#[derive(thiserror::Error, Debug)]
pub enum TransactionError {
    #[error("Lock acquisition timeout: Being {0:?}")]
    LockTimeout(BeingId),

    #[error("Deadlock detected: involving {0:?}")]
    Deadlock(Vec<BeingId>),

    #[error("Validation failed: {0}")]
    Validation(String),

    #[error("Transaction rolled back: tx_id={0}")]
    AlreadyRolledBack(TxId),

    #[error("Transaction committed: tx_id={0}")]
    AlreadyCommitted(TxId),
}

/// Query-related errors
#[derive(thiserror::Error, Debug)]
pub enum QueryError {
    #[error("Invalid query: {0}")]
    Invalid(String),

    #[error("Engine not supported: {engine} — {reason}")]
    UnsupportedEngine { engine: String, reason: String },

    #[error("Aggregate field does not exist: {0}")]
    MissingAggregateField(String),

    #[error("Filter condition error: {0}")]
    Filter(String),

    #[error("Vector dimension mismatch: expected {expected}, actual {actual}")]
    DimensionMismatch { expected: usize, actual: usize },
}

impl From<redb::DatabaseError> for DaoQLError {
    fn from(err: redb::DatabaseError) -> Self {
        DaoQLError::RedbDatabase(err.to_string())
    }
}

impl From<redb::TransactionError> for DaoQLError {
    fn from(err: redb::TransactionError) -> Self {
        DaoQLError::RedbTransaction(err.to_string())
    }
}

impl From<redb::TableError> for DaoQLError {
    fn from(err: redb::TableError) -> Self {
        DaoQLError::RedbDatabase(err.to_string())
    }
}

impl From<redb::CommitError> for DaoQLError {
    fn from(err: redb::CommitError) -> Self {
        DaoQLError::RedbTransaction(err.to_string())
    }
}

impl From<redb::StorageError> for DaoQLError {
    fn from(err: redb::StorageError) -> Self {
        DaoQLError::RedbDatabase(err.to_string())
    }
}

/// Convenience conversion: From<String> for quick error construction
impl From<String> for DaoQLError {
    fn from(s: String) -> Self {
        DaoQLError::InvalidState(s)
    }
}

impl From<&str> for DaoQLError {
    fn from(s: &str) -> Self {
        DaoQLError::InvalidState(s.to_string())
    }
}
