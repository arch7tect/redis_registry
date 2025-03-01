// main.rs
#[macro_use] extern crate rocket;

mod redis_registry;
mod redis_api;
mod redis_registry;

use std::env;
use rocket::http::Status;
use rocket::response::status;
use rocket::serde::json::{Json, Value as JsonValue};
use rocket_okapi::swagger_ui::{make_swagger_ui, SwaggerUIConfig};
use rocket_okapi::okapi::openapi3::OpenApi;
use serde_json::json;

use redis_registry::{AsyncRegistry, RegistryConfig};
use redis_api::{mount_routes, get_openapi_docs};

#[derive(Debug, serde::Serialize)]
struct ApiError {
    error: String,
}

#[catch(404)]
fn not_found() -> status::Custom<Json<ApiError>> {
    status::Custom(Status::NotFound, Json(ApiError {
        error: "Resource was not found.".to_string()
    }))
}

#[catch(500)]
fn internal_error() -> status::Custom<Json<ApiError>> {
    status::Custom(Status::InternalServerError, Json(ApiError {
        error: "Internal server error.".to_string()
    }))
}

#[rocket::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get owner_type and owner_id from environment variables
    let owner_type = env::var("OWNER_TYPE").unwrap_or_else(|_| {
        eprintln!("OWNER_TYPE environment variable not set. Using 'default'");
        "default".to_string()
    });

    let owner_id = env::var("OWNER_ID").unwrap_or_else(|_| {
        eprintln!("OWNER_ID environment variable not set. Using 'default'");
        "default".to_string()
    });

    // Initialize the Redis registry
    let config = RegistryConfig {
        owner_type,
        owner_id,
    };

    let registry = match AsyncRegistry::new(&config) {
        Ok(registry) => registry,
        Err(e) => {
            eprintln!("Failed to initialize Redis registry: {}", e);
            std::process::exit(1);
        }
    };

    // Generate OpenAPI documentation
    let openapi_json = match get_openapi_docs() {
        Ok(docs) => serde_json::to_string_pretty(&docs).unwrap_or_default(),
        Err(e) => {
            eprintln!("Failed to generate OpenAPI documentation: {}", e);
            "{}".to_string()
        }
    };

    // Build and launch the Rocket application
    let rocket_app = rocket::build()
        .manage(registry)
        .register("/", catchers![not_found, internal_error])
        .mount("/openapi.json", rocket::routes![
            || async { openapi_json.clone() }
        ]);

    // Mount Redis registry routes and Swagger UI
    let rocket_app = mount_routes(rocket_app);

    // Launch the application
    if let Err(e) = rocket_app.launch().await {
        eprintln!("Failed to launch Rocket: {}", e);
        std::process::exit(1);
    }

    Ok(())
}