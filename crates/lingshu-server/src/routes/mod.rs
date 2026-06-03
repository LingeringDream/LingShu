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
        description = "灵枢 AI 项目经理个人助理 API"
    ),
    paths(
        system::health_check,
        system::metrics,
        projects::list_projects,
        projects::create_project,
        projects::get_project,
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
    ))
)]
pub struct ApiDoc;

pub fn openapi_spec() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}
