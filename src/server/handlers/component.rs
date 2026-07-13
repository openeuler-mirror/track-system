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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_list_components() {
        let result = list_components().await;
        assert_eq!(result.0.len(), 3);
        assert!(result.0.contains(&"glibc"));
        assert!(result.0.contains(&"gcc"));
        assert!(result.0.contains(&"python"));
    }

    #[test]
    fn test_component_commit_dto_conversion() {
        let commit = ComponentCommit {
            sha: "abc123".to_string(),
            message: "Test commit".to_string(),
            author_name: "John Doe".to_string(),
            author_email: "john@example.com".to_string(),
            authored_at: chrono::Utc::now(),
            url: "https://example.com/commit/abc123".to_string(),
            additions: Some(10),
            deletions: Some(5),
            total: Some(15),
        };

        let dto: ComponentCommitDto = commit.clone().into();
        assert_eq!(dto.sha, commit.sha);
        assert_eq!(dto.message, commit.message);
        assert_eq!(dto.author_name, commit.author_name);
        assert_eq!(dto.author_email, commit.author_email);
        assert_eq!(dto.additions, commit.additions);
        assert_eq!(dto.deletions, commit.deletions);
        assert_eq!(dto.total, commit.total);
    }

    #[tokio::test]
    async fn test_select_client_no_clients() {
        use sea_orm::{DatabaseBackend, MockDatabase};
        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        let state = AppState::without_external_clients(db);

        let result = select_client(&state, None);
        assert!(result.is_err());
        // Since Result::unwrap_err requires T: Debug and (&dyn GitClient, &str) doesn't implement Debug,
        // we can't use unwrap_err() directly here.
        // Instead, we check the error directly from the Result.
        if let Err(e) = result {
            assert_eq!(e, StatusCode::SERVICE_UNAVAILABLE);
        } else {
            panic!("Expected error, but got Ok");
        }
    }

    #[tokio::test]
    async fn test_get_component_handler_error_without_clients() {
        use axum::extract::{Path, Query, State};
        use sea_orm::{DatabaseBackend, MockDatabase};

        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        let state = AppState::without_external_clients(db);

        let params = ComponentQueryParams {
            platform: None,
            owner: None,
            branch: None,
            spec: None,
        };

        let result = get_component(State(state), Path("glibc".to_string()), Query(params)).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), StatusCode::SERVICE_UNAVAILABLE);
    }
    #[tokio::test]
    async fn test_query_components_error_without_clients() {
        use axum::extract::State;
        use sea_orm::{DatabaseBackend, MockDatabase};

        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        let state = AppState::without_external_clients(db);

        let request = ComponentQueryRequest {
            components: vec![ComponentRequest {
                name: "glibc".to_string(),
                platform: None,
                owner: None,
                branch: None,
                spec: None,
            }],
        };

        let result = query_components(State(state), Json(request)).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn test_list_component_commits_error_without_clients() {
        use sea_orm::{DatabaseBackend, MockDatabase};

        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        let state = AppState::without_external_clients(db);
