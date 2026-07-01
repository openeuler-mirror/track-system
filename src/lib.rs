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

#[macro_use]
extern crate rust_i18n;

rust_i18n::i18n!("src/bin/locales", fallback = "en-US");

// Track System - 自动化源码仓库跟踪和分析工具
//
// 模块组织：
// - collectors: 采集器（GitHub, GitLab, Gitee, Gitea, Local）
// - diff: 对比引擎（L1 vs L0, L2 vs L1）
// - snapshot: 快照管理（L0, L1, L2）
// - entities: SeaORM 实体（自动生成）
// - server: Axum Web 服务
// - scheduler: 任务调度
// - analyzer: 分析器
// - utils: 工具函数

// 已实现的模块
pub mod analyzer;
pub mod backport_advisor;
pub mod classifier_job;
pub mod cli;
pub mod collectors;
pub mod component;
pub mod diff;
pub mod entities;
pub mod exporter;
pub mod i18n;
pub mod importer;
// pub mod ingest;  // 已整合到 collectors 和 scheduler，不再需要
pub mod l0;
pub mod metadata_bridge;
pub mod scheduler;
pub mod server;
pub mod snapshot;
pub mod spec;
pub mod telemetry;
pub mod utils;
pub mod workflow;

// 待实现的模块占位符
// pub mod config;
// pub mod repository;
// pub mod branch;
// pub mod concurrency;
// pub mod compare;
// pub mod git;
