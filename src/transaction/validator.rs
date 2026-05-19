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
