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

use crate::telemetry::Telemetry;
/// 元数据导出器
///
/// 功能：
/// - 支持 JSON 和 SQL 格式导出
/// - 支持增量和全量导出
/// - 包含文件完整性校验
use chrono::{DateTime, Utc};
use sea_orm::*;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// 导出格式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExportFormat {
    /// JSON 格式
    Json,
    /// SQL 格式
    Sql,
}

/// 导出选项
#[derive(Debug, Clone)]
pub struct ExportOptions {
    /// 导出格式
    pub format: ExportFormat,
    /// 是否包含 commit 记录
    pub include_commits: bool,
    /// 是否增量导出
    pub incremental: bool,
    /// 增量导出起始时间
    pub since: Option<DateTime<Utc>>,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            format: ExportFormat::Json,
            include_commits: false,
            incremental: false,
            since: None,
        }
    }
}

/// 导出结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportResult {
    /// 是否成功
    pub success: bool,
    /// 导出的软件包数量
    pub exported_packages: usize,
    /// 导出的发行版数量
    pub exported_distros: usize,
    /// 导出的跟踪配置数量
    pub exported_trackings: usize,
    /// 导出的 commit 数量
    pub exported_commits: usize,
    /// 导出时间
    pub export_time: DateTime<Utc>,
    /// 文件校验和
    pub checksum: Option<String>,
    /// 错误信息
    pub error: Option<String>,
}

impl Default for ExportResult {
    fn default() -> Self {
        Self {
            success: false,
            exported_packages: 0,
            exported_distros: 0,
            exported_trackings: 0,
            exported_commits: 0,
            export_time: Utc::now(),
            checksum: None,
            error: None,
        }
    }
}

/// 导出的元数据结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportedMetadata {
    /// 导出时间
    pub export_time: DateTime<Utc>,
    /// 软件包列表
    pub packages: Vec<serde_json::Value>,
    /// 发行版列表
    pub distros: Vec<serde_json::Value>,
    /// 跟踪配置列表
    pub trackings: Vec<serde_json::Value>,
    /// commit 记录列表（可选）
    pub commits: Option<Vec<serde_json::Value>>,
}

/// 元数据导出器
pub struct MetadataExporter<'a> {
    db: &'a DatabaseConnection,
}

impl<'a> MetadataExporter<'a> {
    /// 创建新的导出器
    pub fn new(db: &'a DatabaseConnection) -> Self {
        Self { db }
    }

    /// 导出元数据
    pub async fn export<P: AsRef<Path>>(
        &self,
        path: P,
        options: &ExportOptions,
    ) -> Result<ExportResult, DbErr> {
        let export_time = Utc::now();

        // 查询数据
        let (packages, distros, trackings, commits) = self.fetch_data(options).await?;

        let export_path = path.as_ref();

        // 根据格式导出
        let result = match options.format {
            ExportFormat::Json => {
                self.export_json(
                    export_path,
                    export_time,
                    packages,
                    distros,
                    trackings,
                    commits,
                )
                .await?
            }
            ExportFormat::Sql => {
                self.export_sql(
                    export_path,
                    export_time,
                    packages,
                    distros,
                    trackings,
                    commits,
                )
                .await?
            }
        };

        let path_display = export_path.to_string_lossy();
        Telemetry::snapshot_export_completed(None, path_display.as_ref(), result.export_time);

        Ok(result)
    }

    /// 查询要导出的数据
    async fn fetch_data(
        &self,
        options: &ExportOptions,
    ) -> Result<
        (
            Vec<serde_json::Value>,
            Vec<serde_json::Value>,
            Vec<serde_json::Value>,
            Option<Vec<serde_json::Value>>,
        ),
        DbErr,
    > {
        use crate::entities::{distros, l1_commit_records, packages, tracking};

        // 查询软件包
        let mut packages_query = packages::Entity::find();
        if options.incremental {
            if let Some(since) = options.since {
                packages_query = packages_query.filter(packages::Column::UpdatedAt.gt(since));
            }
        }
        let packages_data = packages_query.all(self.db).await?;
        let packages_json: Vec<serde_json::Value> = packages_data
            .iter()
            .map(|p| {
                serde_json::json!({
                    "id": p.id,
                    "name": p.name,
                    "level": p.level,
                    "sync_interval_hours": p.sync_interval_hours,
                    "created_at": p.created_at,
                    "updated_at": p.updated_at,
                })
            })
            .collect();

        // 查询发行版
        let mut distros_query = distros::Entity::find();
        if options.incremental {
            if let Some(since) = options.since {
                distros_query = distros_query.filter(distros::Column::UpdatedAt.gt(since));
            }
        }
        let distros_data = distros_query.all(self.db).await?;
        let distros_json: Vec<serde_json::Value> = distros_data
            .iter()
            .map(|d| {
                serde_json::json!({
                    "id": d.id,
                    "name": d.name,
                    "version": d.version,
                    "platform": d.platform,
                    "base_url": d.base_url,
                    "created_at": d.created_at,
                    "updated_at": d.updated_at,
                })
            })
            .collect();

        // 查询跟踪配置
        let mut trackings_query = tracking::Entity::find();
        if options.incremental {
            if let Some(since) = options.since {
                trackings_query = trackings_query.filter(tracking::Column::UpdatedAt.gt(since));
            }
        }
        let trackings_data = trackings_query.all(self.db).await?;
        let trackings_json: Vec<serde_json::Value> = trackings_data
            .iter()
            .map(|t| {
                serde_json::json!({
                    "id": t.id,
                    "package_id": t.package_id,
                    "distro_id": t.distro_id,
                    "l1_branch": t.l1_branch,
                    "l1_repo_owner": t.l1_repo_owner,
                    "l1_repo_name": t.l1_repo_name,
                    "l2_branch": t.l2_branch,
                    "l2_repo_path": t.l2_repo_path,
                    "tracking_status": t.tracking_status,
                    "created_at": t.created_at,
                    "updated_at": t.updated_at,
                })
            })
            .collect();

        // 查询 commit 记录（如果需要）
        let commits_json = if options.include_commits {
            let commits_data = l1_commit_records::Entity::find().all(self.db).await?;
            let commits: Vec<serde_json::Value> = commits_data
                .iter()
                .map(|c| {
                    serde_json::json!({
                        "id": c.id,
                        "tracking_id": c.tracking_id,
                        "commit_sha": c.commit_sha,
                        "commit_message": c.commit_message,
                        "author_name": c.author_name,
                        "author_email": c.author_email,
                        "committed_at": c.committed_at,
                        "sync_status": c.sync_status,
                        "api_url": c.api_url,
                        "fetched_at": c.fetched_at,
                        "files_changed_count": c.files_changed_count,
                        "additions": c.additions,
                        "deletions": c.deletions,
                        "created_at": c.created_at,
                        "updated_at": c.updated_at,
                    })
                })
                .collect();
            Some(commits)
        } else {
            None
        };

        Ok((packages_json, distros_json, trackings_json, commits_json))
    }

    /// 导出为 JSON 格式
    async fn export_json(
        &self,
        path: &Path,
        export_time: DateTime<Utc>,
        packages: Vec<serde_json::Value>,
        distros: Vec<serde_json::Value>,
        trackings: Vec<serde_json::Value>,
        commits: Option<Vec<serde_json::Value>>,
    ) -> Result<ExportResult, DbErr> {
        let metadata = ExportedMetadata {
            export_time,
            packages: packages.clone(),
            distros: distros.clone(),
            trackings: trackings.clone(),
            commits: commits.clone(),
        };

        // 序列化为 JSON
        let json_string = serde_json::to_string_pretty(&metadata)
            .map_err(|e| DbErr::Custom(format!("JSON serialization error: {}", e)))?;

        // 写入文件
        std::fs::write(path, &json_string)
            .map_err(|e| DbErr::Custom(format!("File write error: {}", e)))?;

        // 计算校验和
        let checksum = Some(format!("{:x}", md5::compute(&json_string)));

        Ok(ExportResult {
            success: true,
            exported_packages: packages.len(),
            exported_distros: distros.len(),
            exported_trackings: trackings.len(),
            exported_commits: commits.as_ref().map(|c| c.len()).unwrap_or(0),
            export_time,
            checksum,
            error: None,
        })
    }

    /// 导出为 SQL 格式
    async fn export_sql(
        &self,
        path: &Path,
        export_time: DateTime<Utc>,
        packages: Vec<serde_json::Value>,
        distros: Vec<serde_json::Value>,
        trackings: Vec<serde_json::Value>,
        commits: Option<Vec<serde_json::Value>>,
    ) -> Result<ExportResult, DbErr> {
        let mut sql_statements = Vec::new();

        // 生成 packages 的 INSERT 语句
        for pkg in &packages {
            let id = pkg["id"].as_i64().unwrap_or(0);
            let name = pkg["name"].as_str().unwrap_or("");
            let level = pkg["level"].as_i64().unwrap_or(0);
            let sync_interval = pkg["sync_interval_hours"].as_i64().unwrap_or(12);

            sql_statements.push(format!(
                "INSERT INTO packages (id, name, level, sync_interval_hours) VALUES ({}, '{}', {}, {});",
                id, name, level, sync_interval
            ));
