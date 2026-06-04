pub mod auth;
pub mod chat;
pub mod conversations;
pub mod memories;
pub mod projects;
pub mod sessions;
pub mod system;
pub mod tasks;
pub mod users;

use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "LingShu API",
        version = "0.1.0",
        description = "灵枢 macOS 桌面 AI 个人助理 API"
    ),
    paths(
        system::health_check,
        system::metrics,
        projects::list_projects,
        projects::create_project,
        tasks::list_tasks,
        tasks::create_task,
        conversations::list_conversations,
        conversations::create_conversation,
        memories::list_memories,
        memories::search_memories,
    ),
    components(schemas(
        system::HealthResponse,
        projects::CreateProjectRequest,
        projects::ProjectResponse,
        tasks::CreateTaskRequest,
        tasks::TaskResponse,
        conversations::CreateConversationRequest,
        conversations::ConversationResponse,
        memories::MemoryResponse,
    ))
)]
pub struct ApiDoc;

pub fn openapi_spec() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openapi_spec_has_health_check() {
        let spec = openapi_spec();
        let json = serde_json::to_string_pretty(&spec).expect("OpenApi should serialize");
        assert!(
            json.contains("/api/v1/system/health"),
            "OpenAPI spec should include health check endpoint"
        );
        assert!(
            json.contains("LingShu"),
            "OpenAPI spec should include API title"
        );
    }

    #[test]
    fn openapi_spec_has_minimum_paths() {
        let spec = openapi_spec();
        let count = spec.paths.paths.len();
        // Phase 0 registers 7 unique paths (health, metrics, projects, tasks,
        // conversations, memories list, memories search)
        assert!(
            count >= 7,
            "Expected at least 7 registered paths, got {}",
            count
        );
    }
}
