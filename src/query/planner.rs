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


/// Queryplan
#[derive(Debug, Clone)]
pub struct QueryPlan {
    pub steps: Vec<PlanStep>,
}

/// plan step
#[derive(Debug, Clone)]
pub enum PlanStep {
    Scan { def: String, filter: Option<String> },
    Filter { predicate: String },
    Project { fields: Vec<String> },
    Aggregate { op: String, field: String },
    Limit { n: usize },
}

impl QueryPlan {
    pub fn new() -> Self {
        Self { steps: Vec::new() }
    }

    pub fn add_step(&mut self, step: PlanStep) {
        self.steps.push(step);
    }
}

impl Default for QueryPlan {
    fn default() -> Self {
        Self::new()
    }
}
