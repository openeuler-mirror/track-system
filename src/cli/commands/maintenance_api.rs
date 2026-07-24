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

