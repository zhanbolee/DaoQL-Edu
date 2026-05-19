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
