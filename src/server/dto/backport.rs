/*
 * Copyright(c) 2024-2026 China Telecom Cloud Technologies Co., Ltd. All rights
 * reserved. ctscat is licensed under Mulan PSL v2. You can use this software
 * according to the terms and conditions of the Mulan PSL V2. You may obtain a
 * copy of Mulan PSL v2 at: http://license.coscl.org.cn/MulanPSL2.
 * THIS SOFTWARE IS PROVIDED ON AN "AS IS" BASIS, WITHOUT WARRANTIES OF ANY
 * KIND, EITHER EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO NON-INFRINGEMENT,
 * MERCHANTABILITY OR FIT FOR A PARTICULAR PURPOSE.  See the Mulan PSL v2 for
 * more details.
 */

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::entities::backport_candidates;

#[derive(Debug, Serialize)]
pub struct BackportCandidateDto {
    pub id: i64,
    pub package_id: i32,
    pub l0_commit_id: i64,
    pub target_distro_id: i32,
    pub spec_base_version: String,
    pub recommendation: String,
    pub status: String,
    pub patch_artifact: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<backport_candidates::Model> for BackportCandidateDto {
    fn from(model: backport_candidates::Model) -> Self {
        Self {
            id: model.id,
            package_id: model.package_id,
            l0_commit_id: model.l0_commit_id,
            target_distro_id: model.target_distro_id,
            spec_base_version: model.spec_base_version,
            recommendation: model.recommendation,
            status: model.status,
            patch_artifact: model.patch_artifact,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}
