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
