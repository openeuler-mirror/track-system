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

use sea_orm::{DatabaseBackend, Statement};
use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = manager.get_database_backend();
        match backend {
            DatabaseBackend::Sqlite => {
                let conn = manager.get_connection();
                // 关闭外键检查以允许表重建
                conn.execute(Statement::from_string(
                    backend,
                    "PRAGMA foreign_keys=OFF".to_owned(),
                ))
                .await?;

                // 创建不含 distros 外键的新 tracking 表
                manager
                    .create_table(
                        Table::create()
                            .table(Alias::new("tracking_new"))
                            .if_not_exists()
                            .col(pk_auto(Tracking::Id))
                            .col(integer(Tracking::PackageId))
                            .col(integer(Tracking::DistroId))
                            .col(string(Tracking::L1Branch))
                            .col(string(Tracking::L1RepoOwner))
                            .col(string(Tracking::L1RepoName))
                            .col(string(Tracking::L2Branch))
                            .col(string(Tracking::L2RepoPath))
                            .col(string(Tracking::TrackingStatus))
                            .col(timestamp_null(Tracking::LastSyncTime))
                            .col(string_null(Tracking::LastL1CommitSha))
                            .col(string_null(Tracking::LastL2CommitSha))
                            .col(timestamp(Tracking::CreatedAt))
                            .col(timestamp(Tracking::UpdatedAt))
                            .col(ColumnDef::new(Tracking::LastError).text().null())
                            // 保留对 packages 的外键
                            .foreign_key(
                                ForeignKey::create()
                                    .name("fk_tracking_package")
                                    .from(Alias::new("tracking_new"), Tracking::PackageId)
                                    .to(Packages::Table, Packages::Id)
                                    .on_delete(ForeignKeyAction::Cascade),
                            )
                            .to_owned(),
                    )
                    .await?;

                // 迁移数据
                conn
                    .execute(Statement::from_string(
                        backend,
                        "INSERT INTO tracking_new (id, package_id, distro_id, l1_branch, l1_repo_owner, l1_repo_name, l2_branch, l2_repo_path, tracking_status, last_sync_time, last_l1_commit_sha, last_l2_commit_sha, created_at, updated_at, last_error) SELECT id, package_id, distro_id, l1_branch, l1_repo_owner, l1_repo_name, l2_branch, l2_repo_path, tracking_status, last_sync_time, last_l1_commit_sha, last_l2_commit_sha, created_at, updated_at, last_error FROM tracking;".to_owned(),
                    ))
                    .await?;

                // 删除旧表
                conn.execute(Statement::from_string(
                    backend,
                    "DROP TABLE tracking".to_owned(),
                ))
                .await?;

                // 重命名新表
                conn.execute(Statement::from_string(
                    backend,
                    "ALTER TABLE tracking_new RENAME TO tracking".to_owned(),
                ))
                .await?;

                // 重新开启外键检查
                conn.execute(Statement::from_string(
                    backend,
                    "PRAGMA foreign_keys=ON".to_owned(),
                ))
                .await?;
            }
            DatabaseBackend::Postgres => {
                // 删除约束（PostgreSQL）
                manager
                    .get_connection()
                    .execute(Statement::from_string(
                        backend,
                        "ALTER TABLE tracking DROP CONSTRAINT fk_tracking_distro".to_owned(),
                    ))
                    .await?;
            }
            DatabaseBackend::MySql => {
                // 删除约束（MySQL）
                manager
                    .get_connection()
                    .execute(Statement::from_string(
                        backend,
                        "ALTER TABLE tracking DROP FOREIGN KEY fk_tracking_distro".to_owned(),
                    ))
                    .await?;
            }
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = manager.get_database_backend();
        match backend {
            DatabaseBackend::Sqlite => {
                let conn = manager.get_connection();
                // 关闭外键检查以允许表重建
                conn.execute(Statement::from_string(
                    backend,
                    "PRAGMA foreign_keys=OFF".to_owned(),
                ))
                .await?;

                // 创建包含 distros 外键的 tracking 新表
                manager
                    .create_table(
                        Table::create()
                            .table(Alias::new("tracking_new"))
                            .if_not_exists()
                            .col(pk_auto(Tracking::Id))
                            .col(integer(Tracking::PackageId))
                            .col(integer(Tracking::DistroId))
                            .col(string(Tracking::L1Branch))
                            .col(string(Tracking::L1RepoOwner))
                            .col(string(Tracking::L1RepoName))
                            .col(string(Tracking::L2Branch))
                            .col(string(Tracking::L2RepoPath))
                            .col(string(Tracking::TrackingStatus))
                            .col(timestamp_null(Tracking::LastSyncTime))
                            .col(string_null(Tracking::LastL1CommitSha))
                            .col(string_null(Tracking::LastL2CommitSha))
                            .col(timestamp(Tracking::CreatedAt))
                            .col(timestamp(Tracking::UpdatedAt))
                            .col(ColumnDef::new(Tracking::LastError).text().null())
                            // 保留 packages 外键
                            .foreign_key(
                                ForeignKey::create()
                                    .name("fk_tracking_package")
                                    .from(Alias::new("tracking_new"), Tracking::PackageId)
                                    .to(Packages::Table, Packages::Id)
                                    .on_delete(ForeignKeyAction::Cascade),
                            )
                            // 恢复 distros 外键
                            .foreign_key(
                                ForeignKey::create()
                                    .name("fk_tracking_distro")
                                    .from(Alias::new("tracking_new"), Tracking::DistroId)
                                    .to(Distros::Table, Distros::Id)
                                    .on_delete(ForeignKeyAction::Cascade),
                            )
                            .to_owned(),
                    )
                    .await?;

