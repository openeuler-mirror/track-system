use axum::{
    routing::{get, post},
    Router,
};

use crate::server::{handlers::maintenance, state::AppState};

pub fn maintenance_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/maintenance/packages/:id/refresh",
            post(maintenance::refresh_package),
        )
        .route(
            "/maintenance/packages/:id/latest-report",
            get(maintenance::get_latest_report),
        )
        .route("/maintenance/reports", get(maintenance::list_reports))
        .route("/maintenance/reports/:id", get(maintenance::get_report))
}
