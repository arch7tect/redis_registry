// auth.rs
use rocket::http::Status;
use rocket::request::{FromRequest, Outcome};
use rocket::Request;
use std::env;

#[allow(dead_code)]
pub struct ApiKey(pub String);

#[derive(Debug)]
pub enum ApiKeyError {
    Missing,
    Invalid,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for ApiKey {
    type Error = ApiKeyError;

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        // Get the auth token from the environment
        let auth_token = match env::var("AUTH_TOKEN") {
            Ok(token) => token,
            Err(_) => {
                // If the AUTH_TOKEN is not set, authentication is effectively disabled
                warn!("AUTH_TOKEN environment variable not set. API requests will not be authenticated!");
                return Outcome::Success(ApiKey("disabled".to_string()));
            }
        };

        // If authentication is disabled, all requests are allowed
        if auth_token == "disabled" {
            return Outcome::Success(ApiKey("disabled".to_string()));
        }

        // Check if the Authorization header is present
        let auth_header = request.headers().get_one("Authorization");
        match auth_header {
            Some(header) => {
                // Check if it's a Bearer token
                if !header.starts_with("Bearer ") {
                    return Outcome::Error((Status::Unauthorized, ApiKeyError::Invalid));
                }

                // Extract the token
                let token = header[7..].trim();

                // Check if the token matches
                if token == auth_token {
                    return Outcome::Success(ApiKey(token.to_string()));
                } else {
                    return Outcome::Error((Status::Unauthorized, ApiKeyError::Invalid));
                }
            }
            None => Outcome::Error((Status::Unauthorized, ApiKeyError::Missing)),
        }
    }
}