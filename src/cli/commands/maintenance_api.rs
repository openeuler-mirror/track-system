use anyhow::{anyhow, Result};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::cli::client::ApiClient;
use crate::cli::dto::{MaintenanceRefreshResultDto, MaintenanceReportDto, PackageDto};
use crate::cli::formatter::format_datetime_local;
use crate::cli::parser::MaintenanceAction;

#[derive(Debug, Serialize, Deserialize)]
struct ApiResponse<T> {
    data: Option<T>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PaginatedResponse<T> {
    items: Vec<T>,
    total: u64,
    page: u64,
    page_size: u64,
    total_pages: u64,
}

pub async fn execute(api_client: &ApiClient, action: MaintenanceAction) -> Result<()> {
    match action {
        MaintenanceAction::Refresh { package } => {
            let package_id = resolve_package_id(api_client, &package).await?;
            refresh_package(api_client, package_id).await
        }
        MaintenanceAction::LatestReport { package, verbose } => {
            let package_id = resolve_package_id(api_client, &package).await?;
            show_latest_report(api_client, package_id, verbose).await
        }
        MaintenanceAction::Reports {
            page,
            page_size,
            package,
            report_type,
        } => {
            let package_id = match package {
                Some(package) => Some(resolve_package_id(api_client, &package).await?),
                None => None,
            };
            list_reports(api_client, page, page_size, package_id, report_type).await
        }
        MaintenanceAction::Report { id, verbose } => show_report(api_client, id, verbose).await,
    }
