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

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};

use crate::{
    collectors::traits::GitClient,
    component::{
        fetch_component_commits, fetch_component_spec, normalize_spec_path, ComponentCommit,
    },
    server::{
        dto::component::{
            ComponentCommitDto, ComponentCommitParams, ComponentInfo, ComponentQueryParams,
            ComponentQueryRequest, ComponentRequest,
        },
        state::AppState,
    },
};

const PLATFORM_GITEA: &str = "gitea";
const DEFAULT_OWNER_GITEE: &str = "src-openeuler";
const DEFAULT_OWNER_GITEA: &str = "sources-CTyunOS";
const DEFAULT_BRANCH: &str = "master";

pub async fn get_component(
