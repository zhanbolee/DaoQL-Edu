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
//! Constraint Validator
//!
//! Educational Notes:
//! - validate Being whether data conforms to Def Define
//! - check required fields、type、range constraints

use crate::being::Being;
use crate::def::Def;
use crate::error::DaoQLError;

/// Constraintvalidate
pub struct ConstraintValidator;

impl ConstraintValidator {
    /// Validate whether Being conforms to Def
    pub fn validate(being: &Being, def: &Def) -> Result<(), DaoQLError> {
        // validaterequired field
        for field_name in def.required_fields() {
            if being.attr(field_name).is_none() {
                return Err(DaoQLError::ConstraintViolation(format!(
                    "required field '{}' missing",
                    field_name
                )));
            }
        }
        Ok(())
    }
}
