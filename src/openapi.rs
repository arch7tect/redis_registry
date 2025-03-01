// openapi.rs
use utoipa::OpenApi;  // Add this import
use utoipa::openapi::security::{HttpAuthScheme, SecurityScheme};
use utoipa_swagger_ui::SwaggerUi;
use rocket::Rocket;
use rocket::Build;

use crate::redis_registry_api::ApiDoc;

/// Configure the OpenAPI document with security requirements
pub fn configure_openapi() -> utoipa::openapi::OpenApi {
    // Get the generated OpenAPI document
    let mut doc = ApiDoc::openapi();

    // Add security scheme (Bearer authentication)
    let security_scheme = SecurityScheme::Http(
        utoipa::openapi::security::Http::new(HttpAuthScheme::Bearer)
    );

    // Take the existing components or create new ones
    let mut components = doc.components.take().unwrap_or_else(|| utoipa::openapi::Components::default());

    // Add the security scheme
    components.security_schemes.insert("bearer_auth".to_string(), security_scheme);

    // Put the components back
    doc.components = Some(components);

    // Create manually a security requirement Map
    let mut security_reqs = Vec::new();

    // Create a map using the serde_json::Map
    let mut security_map = serde_json::Map::new();
    security_map.insert("bearer_auth".to_string(), serde_json::Value::Array(Vec::new()));

    // Convert to SecurityRequirement through the OpenAPI JSON
    if let Ok(json) = serde_json::to_string(&security_map) {
        if let Ok(req) = serde_json::from_str(&json) {
            security_reqs.push(req);
        }
    }

    // Add security requirement to all operations (global security)
    if !security_reqs.is_empty() {
        doc.security = Some(security_reqs);
    }

    doc
}

/// Mount the Swagger UI with the configured OpenAPI document
pub fn mount_swagger_ui(rocket: Rocket<Build>) -> Rocket<Build> {
    info!("Mounting Swagger UI at /swagger-ui/");
    rocket.mount(
        "/",
        SwaggerUi::new("/swagger-ui/<_..>")
            .url("/api-docs/openapi.json", configure_openapi())
    )
}