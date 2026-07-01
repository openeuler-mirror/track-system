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

        // 提取 CVE 编号并去重
        let mut cve_numbers = Self::extract_cve_numbers(commit_message);
        cve_numbers.sort();
        cve_numbers.dedup();

        // 基于 commit message 确定变更类型
        let primary_type = Self::classify_by_message(commit_message, &cve_numbers);

        // 构建分类结果
        let classification = ChangeClassification {
            cve_numbers,
            primary_type,
            ..Default::default()
        };

        Ok(classification)
    }

    /// 批量分类 commits
    pub async fn batch_classify_commits(
        &self,
        commit_ids: Vec<i32>,
    ) -> Result<Vec<ChangeClassification>> {
        let mut results = Vec::new();
        for commit_id in commit_ids {
            results.push(self.classify_commit(commit_id).await?);
        }
        Ok(results)
    }

    /// 提取 CVE 编号
    fn extract_cve_numbers(text: &str) -> Vec<String> {
        let re = Regex::new(r"CVE-\d{4}-\d+").unwrap();
        re.find_iter(text).map(|m| m.as_str().to_string()).collect()
    }

    /// 基于 commit message 分类变更类型
    fn classify_by_message(message: &str, cve_numbers: &[String]) -> ChangeType {
        let message_lower = message.to_lowercase();

        // 1. 优先识别 CVE（如果提取到 CVE 编号）
