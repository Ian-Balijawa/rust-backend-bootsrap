pub mod file;
pub mod health;
pub mod note;
pub mod scheduler;
pub mod system_config;
pub mod task;
pub mod user;

use crate::AppState;
use axum::Router;
use axum::http::StatusCode;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

pub async fn fallback() -> (StatusCode, &'static str) {
    (StatusCode::NOT_FOUND, "Not Found")
}

pub fn create_router() -> Router<AppState> {
    Router::new()
        .merge(health::router())
        .nest("/api/v1/auth", user::router())
        .nest("/api/v1/tasks", task::router())
        .nest("/api/v1/notes", note::router())
        .nest("/api/v1/files", file::router())
        .nest("/api/v1/scheduler", scheduler::router())
        .nest("/api/v1/system-config", system_config::router())
        .merge(
            SwaggerUi::new("/swagger-ui")
                .url("/api-docs/openapi.json", crate::openapi::ApiDoc::openapi()),
        )
        .fallback(fallback)
}

#[cfg(test)]
mod tests {
    use utoipa::OpenApi;

    #[test]
    fn versioned_router_constructs_with_collection_paths() {
        let _ = super::create_router();
    }

    #[test]
    fn openapi_lists_versioned_routes() {
        let spec = crate::openapi::ApiDoc::openapi();
        let document = serde_json::to_value(&spec).unwrap();
        let paths = spec.paths.paths;
        for path in [
            "/api/v1/auth/signin",
            "/api/v1/auth/refresh",
            "/api/v1/tasks",
            "/api/v1/notes",
            "/api/v1/files",
            "/api/v1/scheduler/tasks",
            "/api/v1/system-config",
            "/health",
            "/ready",
        ] {
            assert!(paths.contains_key(path), "missing OpenAPI path: {path}");
        }
        assert_eq!(
            document["components"]["securitySchemes"]["bearerAuth"]["scheme"],
            "bearer"
        );
        assert!(document["paths"]["/api/v1/notes"]["post"]["requestBody"].is_object());
        assert!(document["paths"]["/api/v1/notes"]["post"]["security"].is_array());
    }
}
