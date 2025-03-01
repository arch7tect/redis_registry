// redis_api.rs
use std::path::PathBuf;
use rocket::http::Status;
use rocket::response::status;
use rocket::serde::json::{Json, Value as JsonValue};
use rocket::{delete, get, post, routes, Route, State};
use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};
use utoipa_swagger_ui::SwaggerUi;

use crate::redis_registry::AsyncRegistry;

// =======================================================
// Response Types
// =======================================================

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ErrorResponse {
    error: String,
}

// =======================================================
// OpenAPI Documentation
// =======================================================

#[derive(OpenApi)]
#[openapi(
    paths(
        set_handler,
        get_handler,
        delete_handler,
        purge_handler,
        scan_handler,
        dump_handler,
        restore_handler
    ),
    components(
        schemas(ErrorResponse),
    ),
)]
pub struct ApiDoc;

// =======================================================
// REST API Handlers
// =======================================================

/// Set a value for the specified key path
#[utoipa::path(
    post,
    path = "/redis/set/{path}",
    params(
        ("path" = String, Path, description = "Key path components")
    ),
    request_body = JsonValue,
    responses(
        (status = 200, description = "Value successfully set", body = String),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[post("/set/<path..>", format = "json", data = "<value>")]
pub async fn set_handler(registry: &State<AsyncRegistry>, path: PathBuf, value: Json<JsonValue>)
                         -> Result<status::Custom<String>, status::Custom<Json<ErrorResponse>>> {

    let parts_str: Vec<&str> = path_to_parts(&path);

    match registry.set(&parts_str, value.into_inner()).await {
        Ok(_) => Ok(status::Custom(Status::Ok, "OK".to_string())),
        Err(e) => Err(status::Custom(Status::InternalServerError, Json(ErrorResponse { error: e.to_string() }))),
    }
}

/// Get a value by its key path
#[utoipa::path(
    get,
    path = "/redis/get/{path}",
    params(
        ("path" = String, Path, description = "Key path components")
    ),
    responses(
        (status = 200, description = "JSON value"),
        (status = 404, description = "Key not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[get("/get/<path..>")]
pub async fn get_handler(registry: &State<AsyncRegistry>, path: PathBuf)
                         -> Result<status::Custom<Json<JsonValue>>, status::Custom<Json<ErrorResponse>>> {

    let parts_str: Vec<&str> = path_to_parts(&path);

    match registry.get(&parts_str).await {
        Ok(Some(value)) => Ok(status::Custom(Status::Ok, Json(value))),
        Ok(None) => Err(status::Custom(Status::NotFound, Json(ErrorResponse { error: "Key not found".to_string() }))),
        Err(e) => Err(status::Custom(Status::InternalServerError, Json(ErrorResponse { error: e.to_string() }))),
    }
}

/// Delete a key by its path
#[utoipa::path(
    delete,
    path = "/redis/{path}",
    params(
        ("path" = String, Path, description = "Key path components")
    ),
    responses(
        (status = 200, description = "Key successfully deleted", body = String),
        (status = 404, description = "Key not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[delete("/<path..>")]
pub async fn delete_handler(registry: &State<AsyncRegistry>, path: PathBuf)
                            -> Result<status::Custom<String>, status::Custom<Json<ErrorResponse>>> {

    let parts_str: Vec<&str> = path_to_parts(&path);

    match registry.delete(&parts_str).await {
        Ok(true) => Ok(status::Custom(Status::Ok, "OK".to_string())),
        Ok(false) => Err(status::Custom(Status::NotFound, Json(ErrorResponse { error: "Key not found".to_string() }))),
        Err(e) => Err(status::Custom(Status::InternalServerError, Json(ErrorResponse { error: e.to_string() }))),
    }
}

/// Purge all keys with the specified prefix
#[utoipa::path(
    post,
    path = "/redis/purge/{path}",
    params(
        ("path" = String, Path, description = "Key path prefix")
    ),
    responses(
        (status = 200, description = "Number of deleted keys", body = String),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[post("/purge/<path..>")]
pub async fn purge_handler(registry: &State<AsyncRegistry>, path: PathBuf)
                           -> Result<status::Custom<String>, status::Custom<Json<ErrorResponse>>> {

    let parts_str: Vec<&str> = path_to_parts(&path);

    match registry.purge(&parts_str).await {
        Ok(count) => Ok(status::Custom(Status::Ok, count.to_string())),
        Err(e) => Err(status::Custom(Status::InternalServerError, Json(ErrorResponse { error: e.to_string() }))),
    }
}

/// Get list of keys with the specified prefix
#[utoipa::path(
    get,
    path = "/redis/scan/{path}",
    params(
        ("path" = String, Path, description = "Key path prefix")
    ),
    responses(
        (status = 200, description = "List of relative key paths"),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[get("/scan/<path..>")]
pub async fn scan_handler(registry: &State<AsyncRegistry>, path: PathBuf)
                          -> Result<status::Custom<Json<Vec<String>>>, status::Custom<Json<ErrorResponse>>> {

    let parts_str: Vec<&str> = path_to_parts(&path);

    match registry.scan(&parts_str).await {
        Ok(keys) => Ok(status::Custom(Status::Ok, Json(keys))),
        Err(e) => Err(status::Custom(Status::InternalServerError, Json(ErrorResponse { error: e.to_string() }))),
    }
}

/// Dump all keys and values with the specified prefix
#[utoipa::path(
    get,
    path = "/redis/dump/{path}",
    params(
        ("path" = String, Path, description = "Key path prefix")
    ),
    responses(
        (status = 200, description = "JSON object with relative keys and values"),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[get("/dump/<path..>")]
pub async fn dump_handler(registry: &State<AsyncRegistry>, path: PathBuf)
                          -> Result<status::Custom<Json<JsonValue>>, status::Custom<Json<ErrorResponse>>> {

    let parts_str: Vec<&str> = path_to_parts(&path);

    match registry.dump(&parts_str).await {
        Ok(data) => Ok(status::Custom(Status::Ok, Json(data))),
        Err(e) => Err(status::Custom(Status::InternalServerError, Json(ErrorResponse { error: e.to_string() }))),
    }
}

/// Restore data from JSON dump
#[utoipa::path(
    post,
    path = "/redis/restore/{path}",
    params(
        ("path" = String, Path, description = "Key path prefix")
    ),
    request_body = JsonValue,
    responses(
        (status = 200, description = "Number of restored keys", body = String),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[post("/restore/<path..>", format = "json", data = "<data>")]
pub async fn restore_handler(registry: &State<AsyncRegistry>, path: PathBuf, data: Json<JsonValue>)
                             -> Result<status::Custom<String>, status::Custom<Json<ErrorResponse>>> {

    let parts_str: Vec<&str> = path_to_parts(&path);

    match registry.restore(&parts_str, data.into_inner()).await {
        Ok(count) => Ok(status::Custom(Status::Ok, count.to_string())),
        Err(e) => Err(status::Custom(Status::InternalServerError, Json(ErrorResponse { error: e.to_string() }))),
    }
}

// Helper function to convert PathBuf to parts vector
fn path_to_parts(path: &PathBuf) -> Vec<&str> {
    path.iter()
        .filter_map(|os_str| match os_str.to_str() {
            Some(s) => {
                let trimmed = s.trim_start().trim_end();
                if trimmed.is_empty() { None } else { Some(trimmed) }
            },
            None => None,
        })
        .collect()
}

// =======================================================
// Route Definitions
// =======================================================

// Function to get api routes
pub fn routes() -> Vec<Route> {
    routes![
        set_handler,
        get_handler,
        delete_handler,
        purge_handler,
        scan_handler,
        dump_handler,
        restore_handler
    ]
}

// Function to mount all routes and swagger docs
pub fn mount_routes(rocket: rocket::Rocket<rocket::Build>) -> rocket::Rocket<rocket::Build> {
    // Regular API routes
    let rocket = rocket.mount("/redis", routes());

    // Mount Swagger UI
    rocket.mount(
        "/",
        SwaggerUi::new("/swagger-ui/<_..>")
            .url("/api-docs/openapi.json", ApiDoc::openapi()),
    )
}