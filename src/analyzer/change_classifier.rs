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
        if !cve_numbers.is_empty() {
            return ChangeType::CVE;
        }

        // 2. 识别 CVE 关键词（即使没有完整的 CVE 编号）
        if message_lower.contains("cve") || message_lower.contains("security") {
            return ChangeType::CVE;
        }

        // 3. 识别 Backport
        let backport_keywords = [
            "backport",
            "cherry-pick",
            "cherry pick",
            "port from",
            "ported from",
            "merge from",
            "merged from",
            "upstream",
        ];
        for keyword in &backport_keywords {
            if message_lower.contains(keyword) {
                return ChangeType::Backport;
            }
        }

        // 4. 识别 Bugfix
        let bugfix_keywords = [
            "fix", "bug", "bugfix", "issue", "problem", "resolve", "patch", "correct",
        ];
        for keyword in &bugfix_keywords {
            if message_lower.contains(keyword) {
                return ChangeType::Bugfix;
            }
        }

        // 5. 默认返回 Unknown
        ChangeType::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::l1_commit_records;
    use sea_orm::MockDatabase;

    #[test]
    fn test_extract_cve_numbers() {
        let text = "Fix CVE-2024-1234 and CVE-2024-5678";
        let cves = ChangeClassifier::extract_cve_numbers(text);
        assert_eq!(cves.len(), 2);
        assert!(cves.contains(&"CVE-2024-1234".to_string()));
        assert!(cves.contains(&"CVE-2024-5678".to_string()));
    }

    #[test]
    fn test_classify_by_message() {
        // CVE
        assert_eq!(
            ChangeClassifier::classify_by_message(
                "Fix CVE-2024-0001",
                &["CVE-2024-0001".to_string()]
            ),
            ChangeType::CVE
        );
        assert_eq!(
            ChangeClassifier::classify_by_message("Fix security vulnerability", &[]),
            ChangeType::CVE
        );

        // Backport
        assert_eq!(
            ChangeClassifier::classify_by_message("Backport from upstream", &[]),
            ChangeType::Backport
        );
        assert_eq!(
            ChangeClassifier::classify_by_message("Cherry-pick commit abc", &[]),
            ChangeType::Backport
        );

        // Bugfix
        assert_eq!(
            ChangeClassifier::classify_by_message("Fix bug #123", &[]),
            ChangeType::Bugfix
        );
        assert_eq!(
            ChangeClassifier::classify_by_message("Resolve issue with login", &[]),
            ChangeType::Bugfix
        );

        // Unknown
        assert_eq!(
            ChangeClassifier::classify_by_message("Update dependencies", &[]),
            ChangeType::Unknown
        );
    }

    #[tokio::test]
    async fn test_classify_commit() {
        let db = MockDatabase::new(sea_orm::DatabaseBackend::Postgres)
            .append_query_results(vec![vec![l1_commit_records::Model {
                id: 1,
                tracking_id: 1,
                commit_sha: "sha1".to_string(),
                commit_message: "Fix CVE-2024-1234".to_string(),
                author_name: "author".to_string(),
                author_email: "author@example.com".to_string(),
                committed_at: chrono::Utc::now(),
                created_at: chrono::Utc::now(),
                change_type: None,
                primary_change_type: None,
                cve_list: None,
                spec_changed: false,
                patch_stats: None,
                classification_status: "pending".to_string(),
                classification_notes: None,
                sync_status: "pending".to_string(),
                synced_to_l2_commit: None,
                synced_at: None,
                api_url: "https://api.github.com/repos/owner/repo/commits/sha1".to_string(),
                fetched_at: chrono::Utc::now(),
                files_changed_count: 1,
                additions: 100,
                deletions: 50,
                updated_at: chrono::Utc::now(),
                spec_version: None,
                spec_release: None,
            }]])
            .into_connection();

        let classifier = ChangeClassifier::new(&db);
        let result = classifier.classify_commit(1).await.unwrap();

        assert_eq!(result.primary_type, ChangeType::CVE);
        assert_eq!(result.cve_numbers, vec!["CVE-2024-1234".to_string()]);
    }
}
