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

/// 变更分类器实现
use super::types::{ChangeClassification, ChangeType};
use crate::entities::prelude::*;
use anyhow::Result;
use regex::Regex;
use sea_orm::*;

/// 变更分类器
pub struct ChangeClassifier<'a> {
    db: &'a DatabaseConnection,
}

impl<'a> ChangeClassifier<'a> {
    /// 创建新的分类器
    pub fn new(db: &'a DatabaseConnection) -> Self {
        Self { db }
    }

    /// 对单个 commit 进行分类
    /// 仅基于 commit message 内容进行分析判断
    pub async fn classify_commit(&self, commit_id: i32) -> Result<ChangeClassification> {
        // 获取 commit 记录
        let commit = L1CommitRecords::find_by_id(commit_id)
            .one(self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Commit not found"))?;

        let commit_message = &commit.commit_message;

