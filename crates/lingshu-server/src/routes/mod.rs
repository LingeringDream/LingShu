pub mod audit;
pub mod auth;
pub mod calendar;
pub mod chat;
pub mod conversations;
pub mod integrations;
pub mod memories;
pub mod permissions;
pub mod personality;
pub mod project_members;
pub mod projects;
pub mod sessions;
pub mod settings;
pub mod system;
pub mod task_dependencies;
pub mod tasks;
pub mod thoughts;
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
        settings::get_llm_settings,
        settings::update_llm_settings,
        projects::list_projects,
        projects::create_project,
        tasks::list_tasks,
        tasks::create_task,
        conversations::list_conversations,
        conversations::create_conversation,
        memories::list_memories,
        memories::get_memory,
        memories::create_memory,
        memories::update_memory,
        memories::delete_memory,
        memories::search_memories,
        calendar::parse_calendar,
        calendar::list_events,
        calendar::create_event,
        calendar::update_event,
        calendar::delete_event,
        permissions::get_permissions,
        permissions::update_permissions,
        project_members::list_members,
        project_members::add_member,
        project_members::get_member,
        project_members::remove_member,
        task_dependencies::list_dependencies,
        task_dependencies::add_dependency,
        task_dependencies::get_dependency,
        task_dependencies::remove_dependency,
        personality::list_snapshots,
        personality::create_snapshot,
        personality::get_active_snapshot,
        personality::activate_snapshot,
        thoughts::list_thoughts,
        thoughts::get_thought,
        thoughts::update_thought,
        integrations::list_integrations,
        integrations::get_integration,
        integrations::delete_integration,
        audit::list_entries,
    ),
    components(schemas(
        system::HealthResponse,
        settings::LlmSettings,
        settings::LlmSettingsPatch,
        projects::CreateProjectRequest,
        projects::ProjectResponse,
        tasks::CreateTaskRequest,
        tasks::UpdateTaskRequest,
        tasks::TaskResponse,
        conversations::CreateConversationRequest,
        conversations::ConversationResponse,
        memories::MemoryResponse,
        memories::CreateMemoryRequest,
        memories::UpdateMemoryRequest,
        calendar::ParsedEvent,
        calendar::EventResponse,
        calendar::CreateEventRequest,
        permissions::PermissionSettings,
        permissions::PermissionPatch,
        project_members::AddMemberRequest,
        project_members::MemberResponse,
        task_dependencies::AddDependencyRequest,
        task_dependencies::DependencyResponse,
        personality::CreateSnapshotRequest,
        personality::SnapshotResponse,
        thoughts::UpdateThoughtRequest,
        thoughts::ThoughtResponse,
        integrations::IntegrationResponse,
        audit::AuditEntryResponse,
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
        // Unique path patterns (utoipa groups methods on the same path):
        // health(1) + metrics(1) + settings(1) + calendar(3) + projects(1) +
        // project-crud(1) + project-health(1) + tasks-list(1) + task-crud(1) +
        // conversations(1) + memories(3) + permissions(1) +
        // project_members(2) + task_dependencies(2) + personality(3) +
        // thoughts(2) + integrations(2) + audit(1) = 28 paths
        assert!(
            count >= 25,
            "Expected at least 25 registered path items, got {count}"
        );
    }
}
