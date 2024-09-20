use std::collections::HashMap;

use crate::{
    checker::check_everything,
    config::{Collection, Config, Mikan},
};
use axum::{
    extract::Query,
    http::StatusCode,
    routing::{any, get, post},
    Json, Router,
};

use http::{header, Method};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

pub async fn serve() {
    let cors = CorsLayer::new()
        .allow_headers(vec![
            header::ACCEPT,
            header::ACCEPT_LANGUAGE,
            header::AUTHORIZATION,
            header::CONTENT_LANGUAGE,
            header::CONTENT_TYPE,
        ])
        .allow_methods(vec![
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::HEAD,
            Method::OPTIONS,
            Method::CONNECT,
            Method::PATCH,
            Method::TRACE,
        ])
        // allow requests from any origin
        .allow_origin(Any);

    // build our application with a single route
    let app = Router::new()
        .route("/", any(|| async { "Hello, World!" }))
        .route("/request-check", post(request_check))
        .route("/mikan", get(find_mikan))
        .route("/add-mikan", post(add_mikan))
        .route("/rm-mikan", post(rm_mikan))
        .route("/collection", get(find_collection))
        .route("/add-collection", post(add_collection))
        .route("/rm-collection", post(rm_collection));

    // run it with hyper on localhost:3000
    let port = 3000;
    let listener = TcpListener::bind(format!("0.0.0.0:{port}")).await.unwrap();
    info!("Listening on port:{port}");
    axum::serve(listener, app.layer(cors)).await.unwrap();
}

#[derive(Serialize)]
struct ApiResponse {
    message: String,
}

fn to_resp(status: StatusCode, message: String) -> (StatusCode, Json<ApiResponse>) {
    (status, Json(ApiResponse { message }))
}

async fn find_mikan(
    Query(params): Query<HashMap<String, String>>,
) -> (StatusCode, Json<Vec<Mikan>>) {
    let config = Config::load().unwrap();
    if let Some(id) = params.get("bangumiId") {
        if let Some(m) = config
            .mikan
            .iter()
            .find(|m| m.url.contains(&format!("bangumiId={id}")))
        {
            return (StatusCode::OK, Json(vec![m.clone()]));
        } else {
            return (StatusCode::NOT_FOUND, Json(vec![]));
        }
    }
    (StatusCode::OK, Json(config.mikan.clone()))
}

async fn add_mikan(Json(m): Json<Mikan>) -> (StatusCode, Json<ApiResponse>) {
    let mut config = Config::load().unwrap();
    match config.add_mikan(m).and_then(|()| config.save()) {
        Err(e) => to_resp(StatusCode::BAD_REQUEST, e.to_string()),
        Ok(_) => to_resp(StatusCode::OK, "add success".to_string()),
    }
}

#[derive(Deserialize)]
struct RmMikanForm {
    url: String,
}

async fn rm_mikan(Json(form): Json<RmMikanForm>) -> (StatusCode, Json<ApiResponse>) {
    let mut config = Config::load().unwrap();
    match config.rm_mikan(&form.url).and_then(|()| config.save()) {
        Err(e) => to_resp(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        Ok(_) => to_resp(StatusCode::OK, "rm sucess".to_string()),
    }
}

async fn find_collection(
    Query(params): Query<HashMap<String, String>>,
) -> (StatusCode, Json<Vec<Collection>>) {
    let config = Config::load().unwrap();
    if let Some(url) = params.get("url") {
        if let Some(c) = config.collections.iter().find(|c| &c.torrent_url == url) {
            return (StatusCode::OK, Json(vec![c.clone()]));
        } else {
            return (StatusCode::NOT_FOUND, Json(vec![]));
        }
    }
    (StatusCode::OK, Json(config.collections.clone()))
}

async fn add_collection(Json(c): Json<Collection>) -> (StatusCode, Json<ApiResponse>) {
    let mut config = Config::load().unwrap();
    match config.add_collection(c).and_then(|()| config.save()) {
        Err(e) => to_resp(StatusCode::BAD_REQUEST, e.to_string()),
        Ok(_) => to_resp(StatusCode::OK, "add success".to_string()),
    }
}

#[derive(Deserialize)]
struct RmCollectionForm {
    url: String,
}

async fn rm_collection(Json(form): Json<RmCollectionForm>) -> (StatusCode, Json<ApiResponse>) {
    let mut config = Config::load().unwrap();
    match config.rm_collection(&form.url).and_then(|()| config.save()) {
        Err(e) => to_resp(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        Ok(_) => to_resp(StatusCode::OK, "rm sucess".to_string()),
    }
}

async fn request_check() -> (StatusCode, Json<ApiResponse>) {
    tokio::spawn(async move { check_everything().await });
    to_resp(StatusCode::OK, "check requested".to_string())
}
