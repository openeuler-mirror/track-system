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
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        println!("Running m20251127_000001_create_compare_reports migration");
        let backend = manager.get_database_backend();

        if backend == DatabaseBackend::Sqlite {
            // SQLite 使用 TEXT 存储 JSON
            let create_sql = r#"
CREATE TABLE IF NOT EXISTS compare_reports (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    tracking_id INTEGER NOT NULL,
    generated_at TEXT NOT NULL,
    l2_vs_l1_diff TEXT,
    l1_vs_l0_diff TEXT,
    status TEXT NOT NULL,
    failure_reason TEXT,
    created_at TEXT NOT NULL,
