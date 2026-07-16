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

pub use sea_orm_migration::prelude::*;

mod m20251014_060104_create_packages;
mod m20251014_060133_create_commit_files;
mod m20251014_060133_create_commit_records;
mod m20251014_060133_create_distributed_locks;
mod m20251014_060133_create_distros;
mod m20251014_060133_create_spec_changes;
mod m20251014_060133_create_spec_snapshots;
mod m20251014_060133_create_tracking;
mod m20251014_070628_add_last_error_to_tracking;
mod m20251014_073426_add_commit_file_change_details;
mod m20251014_122500_add_app_release_layers;
mod m20251110_create_audit_logs;
mod m20251121_090001_drop_tracking_distro_fk;
mod m20251125_000001_add_spec_fields_to_commit_records;
mod m20251125_060001_rename_commit_records_to_l1;
mod m20251125_060002_create_l2_commit_records;
mod m20251127_000001_create_compare_reports;
mod m20260202_000001_add_platform_to_tracking;
mod m20260331_000001_create_ecosystem_targets;
mod m20260331_000002_create_ecosystem_bindings;
mod m20260331_000003_create_ecosystem_evidence_snapshots;
mod m20260331_000004_create_ecosystem_reports;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20251014_060104_create_packages::Migration),
            Box::new(m20251014_060133_create_commit_files::Migration),
            Box::new(m20251014_060133_create_commit_records::Migration),
            Box::new(m20251014_060133_create_distributed_locks::Migration),
            Box::new(m20251014_060133_create_distros::Migration),
            Box::new(m20251014_060133_create_spec_changes::Migration),
            Box::new(m20251014_060133_create_spec_snapshots::Migration),
            Box::new(m20251014_060133_create_tracking::Migration),
            Box::new(m20251014_070628_add_last_error_to_tracking::Migration),
            Box::new(m20251014_073426_add_commit_file_change_details::Migration),
            Box::new(m20251014_122500_add_app_release_layers::Migration),
            Box::new(m20251110_create_audit_logs::Migration),
            Box::new(m20251121_090001_drop_tracking_distro_fk::Migration),
            Box::new(m20251125_000001_add_spec_fields_to_commit_records::Migration),
            Box::new(m20251125_060001_rename_commit_records_to_l1::Migration),
            Box::new(m20251125_060002_create_l2_commit_records::Migration),
            Box::new(m20251127_000001_create_compare_reports::Migration),
            Box::new(m20260202_000001_add_platform_to_tracking::Migration),
            Box::new(m20260331_000001_create_ecosystem_targets::Migration),
            Box::new(m20260331_000002_create_ecosystem_bindings::Migration),
            Box::new(m20260331_000003_create_ecosystem_evidence_snapshots::Migration),
            Box::new(m20260331_000004_create_ecosystem_reports::Migration),
        ]
    }
}
