// Copyright 2026 Zhanbo Li / Atlas Lee <4859345@qq.com>
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

use crate::column::skip_index::GranuleMeta;

/// Data Granule
pub struct Granule {
    pub meta: GranuleMeta,
    pub data: Vec<u8>,
}

impl Granule {
    pub fn new(offset: u64) -> Self {
        Self {
            meta: GranuleMeta::new(offset, 0),
            data: Vec::with_capacity(64 * 1024),
        }
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}
