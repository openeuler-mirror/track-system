use axum::{
    routing::{delete, get, post, put},
    Router,
};

use crate::server::{handlers::ecosystem, state::AppState};

pub fn ecosystem_routes() -> Router<AppState> {
    Router::new()
        .route("/ecosystem/targets", get(ecosystem::list_targets))
        .route("/ecosystem/targets", post(ecosystem::create_target))
        .route("/ecosystem/targets/:id", get(ecosystem::get_target))
        .route("/ecosystem/targets/:id", put(ecosystem::update_target))
        .route("/ecosystem/targets/:id", delete(ecosystem::delete_target))
        .route(
            "/ecosystem/targets/:id/refresh",
            post(ecosystem::refresh_target),
        )
