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

//! 各个命令的实现模块
//!
//! 所有命令现在都通过 API 客户端与 track-server 通信
//! 不再直接连接数据库

// API 客户端命令（新架构）
pub mod classify_api;
pub mod compare_api;
pub mod config_api;
pub mod distro_api;
pub mod export_api;
pub mod health_api;
pub mod import_api;
pub mod l0_api;
pub mod package_api;
pub mod report_api;
pub mod server;
pub mod snapshot_api;
pub mod status_api;
pub mod sync_api;
pub mod tracking_api;
pub mod workflow_api;
