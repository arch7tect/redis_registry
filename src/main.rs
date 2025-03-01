// main.rs
#[macro_use] extern crate rocket;
#[macro_use] extern crate tracing;
extern crate dotenv;

mod redis_registry;
mod redis_registry_api;
mod auth;
mod openapi;

use std::env;
use std::io;
use rocket::http::Status;
use rocket::response::status;
use rocket::serde::json::Json;
use dotenv::dotenv;
use tracing_subscriber::{
    fmt,
    EnvFilter,
    prelude::*,
    layer::SubscriberExt,
};
use tracing_appender::{non_blocking, rolling};

use redis_registry::{AsyncRegistry, RegistryConfig};
use redis_registry_api::mount_routes;
use openapi::mount_swagger_ui;

#[derive(Debug, serde::Serialize)]
struct ApiError {
    error: String,
}

#[catch(404)]
fn not_found() -> status::Custom<Json<ApiError>> {
    error!("Resource not found");
    status::Custom(Status::NotFound, Json(ApiError {
        error: "Resource was not found.".to_string()
    }))
}

#[catch(500)]
fn internal_error() -> status::Custom<Json<ApiError>> {
    error!("Internal server error");
    status::Custom(Status::InternalServerError, Json(ApiError {
        error: "Internal server error.".to_string()
    }))
}

#[catch(401)]
fn unauthorized() -> status::Custom<Json<ApiError>> {
    error!("Unauthorized access attempt");
    status::Custom(Status::Unauthorized, Json(ApiError {
        error: "Authentication required.".to_string()
    }))
}

fn setup_logging() -> io::Result<()> {
    // Get log level from environment variable or use default
    let log_level = env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());

    // Get log directory from environment variable or use default
    let log_dir = env::var("LOG_DIR").unwrap_or_else(|_| "logs".to_string());

    // Create log directory if it doesn't exist
    std::fs::create_dir_all(&log_dir)?;

    // Configure file appender for rotating log files daily
    let file_appender = rolling::daily(&log_dir, "registry-api");
    let (non_blocking_appender, _guard) = non_blocking(file_appender);

    // Store the guard in a static to keep it alive for the duration of the program
    // This prevents the non-blocking writer from being dropped prematurely
    static mut GUARD: Option<tracing_appender::non_blocking::WorkerGuard> = None;
    unsafe {
        GUARD = Some(_guard);
    }

    // Create console layer for stdout
    let console_layer = fmt::layer()
        .with_target(true)
        .with_ansi(true);

    // Create JSON-formatted file layer
    let file_layer = fmt::layer()
        .with_ansi(false)
        .with_writer(non_blocking_appender)
        .json();

    // Create environment filter from log level
    let env_filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(&log_level))
        .unwrap();

    // Combine all layers
    tracing_subscriber::registry()
        .with(env_filter)
        .with(console_layer)
        .with(file_layer)
        .init();

    info!("Logging initialized with level: {}", log_level);
    Ok(())
}

#[rocket::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables from .env file
    dotenv().ok();

    // Set up logging
    if let Err(e) = setup_logging() {
        eprintln!("Failed to initialize logging: {}", e);
    }

    // Print configuration information
    if let Ok(port) = env::var("ROCKET_PORT") {
        info!("Server will start on port: {}", port);
    } else {
        info!("Server will start on default port 8000");
    }

    // Get owner_type and owner_id from environment variables
    let owner_type = env::var("OWNER_TYPE").unwrap_or_else(|_| {
        warn!("OWNER_TYPE environment variable not set. Using 'default'");
        "default".to_string()
    });

    let owner_id = env::var("OWNER_ID").unwrap_or_else(|_| {
        warn!("OWNER_ID environment variable not set. Using 'default'");
        "default".to_string()
    });

    // Check for authentication token
    let auth_token = env::var("AUTH_TOKEN").unwrap_or_else(|_| {
        warn!("AUTH_TOKEN environment variable not set. API requests will not be authenticated!");
        "disabled".to_string()
    });

    if auth_token == "disabled" {
        warn!("Authentication is disabled. API endpoints are unprotected!");
    } else {
        info!("API endpoints are protected with bearer token authentication");
    }

    info!("Registry initialized with owner_type={}, owner_id={}", owner_type, owner_id);

    // Initialize the Redis registry
    let config = RegistryConfig {
        owner_type,
        owner_id,
    };

    let registry = match AsyncRegistry::new(&config) {
        Ok(registry) => {
            info!("Redis registry successfully initialized");
            registry
        },
        Err(e) => {
            error!("Failed to initialize Redis registry: {}", e);
            std::process::exit(1);
        }
    };

    // Build and launch the Rocket application
    info!("Starting Rocket application...");
    let rocket_app = rocket::build()
        .manage(registry)
        .register("/", catchers![not_found, internal_error, unauthorized]);

    // Mount Redis registry routes
    let rocket_app = mount_routes(rocket_app);

    // Mount Swagger UI
    let rocket_app = mount_swagger_ui(rocket_app);

    // Launch the application
    info!("Launching Rocket application");
    if let Err(e) = rocket_app.launch().await {
        error!("Failed to launch Rocket: {}", e);
        std::process::exit(1);
    }

    Ok(())
}