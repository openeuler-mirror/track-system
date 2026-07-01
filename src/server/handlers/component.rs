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
    State(state): State<AppState>,
    Path(name): Path<String>,
    Query(params): Query<ComponentQueryParams>,
) -> Result<Json<ComponentInfo>, StatusCode> {
    let (client, owner_default) = select_client(&state, params.platform.as_deref())?;
    let owner = params.owner.as_deref().unwrap_or(owner_default);
    let branch = params.branch.as_deref().unwrap_or(DEFAULT_BRANCH);
    let spec_path = normalize_spec_path(&name, params.spec.as_deref());

    let spec = fetch_component_spec(client, owner, &name, branch, &spec_path)
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

    Ok(Json(ComponentInfo {
        name: spec.name,
        version: spec.version,
        release: spec.release,
    }))
}

pub async fn list_components() -> Json<Vec<&'static str>> {
    Json(vec!["glibc", "gcc", "python"])
}

pub async fn query_components(
    State(state): State<AppState>,
    Json(body): Json<ComponentQueryRequest>,
) -> Result<Json<Vec<ComponentInfo>>, StatusCode> {
    let mut results = Vec::with_capacity(body.components.len());

    for component in body.components {
        let info = handle_single_component(&state, component).await?;
        results.push(info);
    }

    Ok(Json(results))
}

async fn handle_single_component(
    state: &AppState,
    request: ComponentRequest,
) -> Result<ComponentInfo, StatusCode> {
    let (client, owner_default) = select_client(state, request.platform.as_deref())?;
    let owner = request.owner.as_deref().unwrap_or(owner_default);
    let branch = request.branch.as_deref().unwrap_or(DEFAULT_BRANCH);
    let spec_path = normalize_spec_path(&request.name, request.spec.as_deref());

    let spec = fetch_component_spec(client, owner, &request.name, branch, &spec_path)
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

    Ok(ComponentInfo {
        name: spec.name,
        version: spec.version,
        release: spec.release,
    })
}

pub async fn list_component_commits(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Query(params): Query<ComponentCommitParams>,
) -> Result<Json<Vec<ComponentCommitDto>>, StatusCode> {
    let (client, owner_default) = select_client(&state, params.platform.as_deref())?;
    let owner = params.owner.as_deref().unwrap_or(owner_default);
    let branch = params.branch.as_deref().unwrap_or(DEFAULT_BRANCH);
    let commits =
        fetch_component_commits(client, owner, &name, branch, params.page, params.per_page)
            .await
            .map_err(|_| StatusCode::BAD_GATEWAY)?;

    Ok(Json(
        commits.into_iter().map(ComponentCommitDto::from).collect(),
    ))
}

fn select_client<'a>(
    state: &'a AppState,
    platform: Option<&str>,
) -> Result<(&'a dyn GitClient, &'static str), StatusCode> {
    match platform.map(|p| p.to_ascii_lowercase()) {
        Some(ref p) if p == PLATFORM_GITEA => {
            let client = state
                .gitea
                .as_ref()
                .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
            Ok((client.as_ref(), DEFAULT_OWNER_GITEA))
        }
        _ => {
            let client = state
                .gitee
                .as_ref()
                .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
            Ok((client.as_ref(), DEFAULT_OWNER_GITEE))
        }
    }
}

impl From<ComponentCommit> for ComponentCommitDto {
    fn from(commit: ComponentCommit) -> Self {
        Self {
            sha: commit.sha,
            message: commit.message,
            author_name: commit.author_name,
            author_email: commit.author_email,
            authored_at: commit.authored_at,
            url: commit.url,
            additions: commit.additions,
            deletions: commit.deletions,
            total: commit.total,
        }
    }
}

