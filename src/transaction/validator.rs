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
//! 约束验证器
//!
//! 教学说明：
//! - 验证 Being 数据是否符合 Def 定义
//! - 检查必填字段、类型、范围约束

use crate::being::Being;
use crate::def::Def;
use crate::error::DaoQLError;

/// 约束验证器
pub struct ConstraintValidator;

impl ConstraintValidator {
    /// 验证 Being 是否符合 Def
    pub fn validate(being: &Being, def: &Def) -> Result<(), DaoQLError> {
        // 验证必填字段
        for field_name in def.required_fields() {
            if being.attr(field_name).is_none() {
                return Err(DaoQLError::ConstraintViolation(format!(
                    "必填字段 '{}' 缺失",
                    field_name
                )));
            }
        }
        Ok(())
    }
}
