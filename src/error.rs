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
//! 统一错误类型
//!
//! 教学说明：
//! - 使用 thiserror 简化错误枚举定义
//! - 所有错误统一收敛到 DaoQLError，避免裸 Box<dyn Error>
//! - 错误信息使用中文，便于教学理解

use std::path::PathBuf;

use crate::id::BeingId;
use crate::transaction::TxId;

/// DaoQL-Edu 统一错误类型
#[derive(thiserror::Error, Debug)]
pub enum DaoQLError {
    #[error("存储错误: {0}")]
    Storage(#[from] StorageError),

    #[error("索引错误: {0}")]
    Index(#[from] IndexError),

    #[error("事务错误: {0}")]
    Transaction(#[from] TransactionError),

    #[error("查询错误: {0}")]
    Query(#[from] QueryError),

    #[error("DSL 解析错误: {0}")]
    DslParse(String),

    #[error("配置错误: {0}")]
    Config(String),

    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("redb 数据库错误: {0}")]
    RedbDatabase(String),

    #[error("redb 事务错误: {0}")]
    RedbTransaction(String),

    #[error("序列化错误: {0}")]
    Serialization(#[from] postcard::Error),

    #[error("JSON 错误: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Not Found: Being {0:?}")]
    NotFound(BeingId),

    #[error("类型不匹配: 期望 {expected}, 实际 {actual}")]
    TypeMismatch { expected: String, actual: String },

    #[error("约束违反: {0}")]
    ConstraintViolation(String),

    #[error("MVCC 冲突: 事务 {tx_id} 读取了未提交数据")]
    MvccConflict { tx_id: TxId },

    #[error("WAL 错误: {0}")]
    Wal(String),

    #[error("非法状态: {0}")]
    InvalidState(String),

    #[error("不支持的特性: {0}")]
    Unsupported(String),

    #[error("资源不足: {0}")]
    ResourceExhausted(String),

    #[error("路径错误: {path} — {reason}")]
    Path { path: PathBuf, reason: String },
}

/// 存储相关错误
#[derive(thiserror::Error, Debug)]
pub enum StorageError {
    #[error("mmap 失败: {0}")]
    Mmap(String),

    #[error("文件打开失败: {path} — {source}")]
    FileOpen { path: PathBuf, source: std::io::Error },

    #[error("记录大小不匹配: 期望 {expected}, 实际 {actual}")]
    RecordSizeMismatch { expected: usize, actual: usize },

    #[error("偏移越界: offset={offset}, capacity={capacity}")]
    OffsetOutOfBounds { offset: usize, capacity: usize },

    #[error("容量不足: 需要 {need}, 剩余 {remaining}")]
    CapacityExceeded { need: usize, remaining: usize },

    #[error("校验失败: {0}")]
    Corruption(String),
}

/// 索引相关错误
#[derive(thiserror::Error, Debug)]
pub enum IndexError {
    #[error("redb 错误: {0}")]
    Redb(#[from] redb::Error),

    #[error("键不存在: {0}")]
    KeyNotFound(String),

    #[error("重复键: {0}")]
    DuplicateKey(String),

    #[error("范围查询错误: {0}")]
    RangeQuery(String),
}

/// 事务相关错误
#[derive(thiserror::Error, Debug)]
pub enum TransactionError {
    #[error("锁获取超时: Being {0:?}")]
    LockTimeout(BeingId),

    #[error("死锁检测: 涉及 {0:?}")]
    Deadlock(Vec<BeingId>),

    #[error("验证失败: {0}")]
    Validation(String),

    #[error("事务已回滚: tx_id={0}")]
    AlreadyRolledBack(TxId),

    #[error("事务已提交: tx_id={0}")]
    AlreadyCommitted(TxId),
}

/// 查询相关错误
#[derive(thiserror::Error, Debug)]
pub enum QueryError {
    #[error("非法查询: {0}")]
    Invalid(String),

    #[error("引擎不支持: {engine} — {reason}")]
    UnsupportedEngine { engine: String, reason: String },

    #[error("聚合字段不存在: {0}")]
    MissingAggregateField(String),

    #[error("过滤条件错误: {0}")]
    Filter(String),

    #[error("向量维度不匹配: 期望 {expected}, 实际 {actual}")]
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

/// 便捷转换：From<String> 用于快速构造错误
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
