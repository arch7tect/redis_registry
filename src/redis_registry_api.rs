// registry_api.rs
use rocket::http::Status;
use rocket::response::status;
use rocket::serde::json::{Json, Value as JsonValue};
use rocket::{delete, get, post, routes, Route, State};
use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};

use crate::redis_registry::AsyncRegistry;
use crate::auth::ApiKey;

// =======================================================
// Response Types
// =======================================================

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ErrorResponse {
    pub error: String,
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
        schemas(ErrorResponse)
    ),
    tags(
        (name = "registry", description = "Registry API")
    )
)]
pub struct ApiDoc;

// =======================================================
// REST API Handlers
// =======================================================

/// Set a value for the specified key path
#[utoipa::path(
    post,
    path = "/registry/set",
    tag = "registry",
    params(
        ("path" = Option<String>, Query, description = "Key path as a string (can be empty or nested using forward slashes like 'a/b/c')")
    ),
    request_body = JsonValue,
    responses(
        (status = 200, description = "Value successfully set", body = String),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[post("/set?<path>", format = "json", data = "<value>")]
pub async fn set_handler(_api_key: ApiKey, registry: &State<AsyncRegistry>, path: Option<String>, value: Json<JsonValue>)
                         -> Result<status::Custom<String>, status::Custom<Json<ErrorResponse>>> {
    debug!("Set request received for path: {:?}", path);
    let span = info_span!("set_handler", path = ?path);
    let _guard = span.enter();

    let parts = path_to_parts(path.clone());
    // Convert Vec<String> to Vec<&str> for the registry functions
    let parts_str: Vec<&str> = parts.iter().map(|s| s.as_str()).collect();

    match registry.set(&parts_str, value.into_inner()).await {
        Ok(_) => {
            info!("Value set successfully for path: {:?}", path);
            Ok(status::Custom(Status::Ok, "OK".to_string()))
        },
        Err(e) => {
            error!("Failed to set value for path {:?}: {}", path, e);
            Err(status::Custom(Status::InternalServerError, Json(ErrorResponse { error: e.to_string() })))
        },
    }
}

/// Get a value by its key path
#[utoipa::path(
    get,
    path = "/registry/get",
    tag = "registry",
    params(
        ("path" = Option<String>, Query, description = "Key path as a string (can be empty or nested using forward slashes like 'a/b/c')")
    ),
    responses(
        (status = 200, description = "JSON value"),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "Key not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[get("/get?<path>")]
pub async fn get_handler(_api_key: ApiKey, registry: &State<AsyncRegistry>, path: Option<String>)
                         -> Result<status::Custom<Json<JsonValue>>, status::Custom<Json<ErrorResponse>>> {
    debug!("Get request received for path: {:?}", path);
    let span = info_span!("get_handler", path = ?path);
    let _guard = span.enter();

    let parts = path_to_parts(path.clone());
    // Convert Vec<String> to Vec<&str> for the registry functions
    let parts_str: Vec<&str> = parts.iter().map(|s| s.as_str()).collect();

    match registry.get(&parts_str).await {
        Ok(Some(value)) => {
            info!("Value found for path: {:?}", path);
            Ok(status::Custom(Status::Ok, Json(value)))
        },
        Ok(None) => {
            warn!("Key not found for path: {:?}", path);
            Err(status::Custom(Status::NotFound, Json(ErrorResponse { error: "Key not found".to_string() })))
        },
        Err(e) => {
            error!("Failed to get value for path {:?}: {}", path, e);
            Err(status::Custom(Status::InternalServerError, Json(ErrorResponse { error: e.to_string() })))
        },
    }
}

/// Delete a key by its path
#[utoipa::path(
    delete,
    path = "/registry/delete",
    tag = "registry",
    params(
        ("path" = Option<String>, Query, description = "Key path as a string (can be empty or nested using forward slashes like 'a/b/c')")
    ),
    responses(
        (status = 200, description = "Key successfully deleted", body = String),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 404, description = "Key not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[delete("/delete?<path>")]
pub async fn delete_handler(_api_key: ApiKey, registry: &State<AsyncRegistry>, path: Option<String>)
                            -> Result<status::Custom<String>, status::Custom<Json<ErrorResponse>>> {
    debug!("Delete request received for path: {:?}", path);
    let span = info_span!("delete_handler", path = ?path);
    let _guard = span.enter();

    let parts = path_to_parts(path.clone());
    // Convert Vec<String> to Vec<&str> for the registry functions
    let parts_str: Vec<&str> = parts.iter().map(|s| s.as_str()).collect();

    match registry.delete(&parts_str).await {
        Ok(true) => {
            info!("Key deleted successfully for path: {:?}", path);
            Ok(status::Custom(Status::Ok, "OK".to_string()))
        },
        Ok(false) => {
            warn!("Key not found for deletion at path: {:?}", path);
            Err(status::Custom(Status::NotFound, Json(ErrorResponse { error: "Key not found".to_string() })))
        },
        Err(e) => {
            error!("Failed to delete key at path {:?}: {}", path, e);
            Err(status::Custom(Status::InternalServerError, Json(ErrorResponse { error: e.to_string() })))
        },
    }
}

/// Purge all keys with the specified prefix
#[utoipa::path(
    post,
    path = "/registry/purge",
    tag = "registry",
    params(
        ("path" = Option<String>, Query, description = "Key path prefix as a string (can be empty or nested using forward slashes like 'a/b/c')")
    ),
    responses(
        (status = 200, description = "Number of deleted keys", body = String),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[post("/purge?<path>")]
pub async fn purge_handler(_api_key: ApiKey, registry: &State<AsyncRegistry>, path: Option<String>)
                           -> Result<status::Custom<String>, status::Custom<Json<ErrorResponse>>> {
    debug!("Purge request received for path prefix: {:?}", path);
    let span = info_span!("purge_handler", path = ?path);
    let _guard = span.enter();

    let parts = path_to_parts(path.clone());
    // Convert Vec<String> to Vec<&str> for the registry functions
    let parts_str: Vec<&str> = parts.iter().map(|s| s.as_str()).collect();

    match registry.purge(&parts_str).await {
        Ok(count) => {
            info!("Purged {} keys with prefix: {:?}", count, path);
            Ok(status::Custom(Status::Ok, count.to_string()))
        },
        Err(e) => {
            error!("Failed to purge keys with prefix {:?}: {}", path, e);
            Err(status::Custom(Status::InternalServerError, Json(ErrorResponse { error: e.to_string() })))
        },
    }
}

/// Get list of keys with the specified prefix
#[utoipa::path(
    get,
    path = "/registry/scan",
    tag = "registry",
    params(
        ("path" = Option<String>, Query, description = "Key path prefix as a string (can be empty or nested using forward slashes like 'a/b/c')")
    ),
    responses(
        (status = 200, description = "List of relative key paths"),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[get("/scan?<path>")]
pub async fn scan_handler(_api_key: ApiKey, registry: &State<AsyncRegistry>, path: Option<String>)
                          -> Result<status::Custom<Json<Vec<String>>>, status::Custom<Json<ErrorResponse>>> {
    debug!("Scan request received for path prefix: {:?}", path);
    let span = info_span!("scan_handler", path = ?path);
    let _guard = span.enter();

    let parts = path_to_parts(path.clone());
    // Convert Vec<String> to Vec<&str> for the registry functions
    let parts_str: Vec<&str> = parts.iter().map(|s| s.as_str()).collect();

    match registry.scan(&parts_str).await {
        Ok(keys) => {
            info!("Found {} keys with prefix: {:?}", keys.len(), path);
            debug!("Keys found: {:?}", keys);
            Ok(status::Custom(Status::Ok, Json(keys)))
        },
        Err(e) => {
            error!("Failed to scan keys with prefix {:?}: {}", path, e);
            Err(status::Custom(Status::InternalServerError, Json(ErrorResponse { error: e.to_string() })))
        },
    }
}

/// Dump all keys and values with the specified prefix
#[utoipa::path(
    get,
    path = "/registry/dump",
    tag = "registry",
    params(
        ("path" = Option<String>, Query, description = "Key path prefix as a string (can be empty or nested using forward slashes like 'a/b/c')")
    ),
    responses(
        (status = 200, description = "JSON object with relative keys and values"),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[get("/dump?<path>")]
pub async fn dump_handler(_api_key: ApiKey, registry: &State<AsyncRegistry>, path: Option<String>)
                          -> Result<status::Custom<Json<JsonValue>>, status::Custom<Json<ErrorResponse>>> {
    debug!("Dump request received for path prefix: {:?}", path);
    let span = info_span!("dump_handler", path = ?path);
    let _guard = span.enter();

    let parts = path_to_parts(path.clone());
    // Convert Vec<String> to Vec<&str> for the registry functions
    let parts_str: Vec<&str> = parts.iter().map(|s| s.as_str()).collect();

    match registry.dump(&parts_str).await {
        Ok(data) => {
            let count = match &data {
                JsonValue::Object(map) => map.len(),
                _ => 0,
            };
            info!("Dumped {} keys with prefix: {:?}", count, path);
            Ok(status::Custom(Status::Ok, Json(data)))
        },
        Err(e) => {
            error!("Failed to dump keys with prefix {:?}: {}", path, e);
            Err(status::Custom(Status::InternalServerError, Json(ErrorResponse { error: e.to_string() })))
        },
    }
}

/// Restore data from JSON dump
#[utoipa::path(
    post,
    path = "/registry/restore",
    tag = "registry",
    params(
        ("path" = Option<String>, Query, description = "Key path prefix as a string (can be empty or nested using forward slashes like 'a/b/c')")
    ),
    request_body = JsonValue,
    responses(
        (status = 200, description = "Number of restored keys", body = String),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    )
)]
#[post("/restore?<path>", format = "json", data = "<data>")]
pub async fn restore_handler(_api_key: ApiKey, registry: &State<AsyncRegistry>, path: Option<String>, data: Json<JsonValue>)
                             -> Result<status::Custom<String>, status::Custom<Json<ErrorResponse>>> {
    debug!("Restore request received for path prefix: {:?}", path);
    let span = info_span!("restore_handler", path = ?path);
    let _guard = span.enter();

    let parts = path_to_parts(path.clone());
    // Convert Vec<String> to Vec<&str> for the registry functions
    let parts_str: Vec<&str> = parts.iter().map(|s| s.as_str()).collect();

    match registry.restore(&parts_str, data.into_inner()).await {
        Ok(count) => {
            info!("Restored {} keys with prefix: {:?}", count, path);
            Ok(status::Custom(Status::Ok, count.to_string()))
        },
        Err(e) => {
            error!("Failed to restore keys with prefix {:?}: {}", path, e);
            Err(status::Custom(Status::InternalServerError, Json(ErrorResponse { error: e.to_string() })))
        },
    }
}

// Helper function to convert path string to parts vector
fn path_to_parts(path: Option<String>) -> Vec<String> {
    match path {
        Some(p) if !p.trim().is_empty() => {
            p.split('/')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        },
        _ => Vec::new()
    }
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

// Function to mount API routes
pub fn mount_routes(rocket: rocket::Rocket<rocket::Build>) -> rocket::Rocket<rocket::Build> {
    // Regular API routes
    rocket.mount("/registry", routes())
}