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
pub mod signals;
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
        auth::local_session,
        users::get_me,
        users::update_me,
        settings::get_llm_settings,
        settings::update_llm_settings,
        projects::list_projects,
        projects::create_project,
        projects::get_project,
        projects::update_project,
        projects::delete_project,
        projects::get_health,
        tasks::list_tasks,
        tasks::create_task,
        tasks::get_task,
        tasks::update_task,
        tasks::delete_task,
        conversations::list_conversations,
        conversations::create_conversation,
        conversations::get_conversation,
        conversations::delete_conversation,
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
        calendar::confirm_event,
        calendar::save_apple_event_id,
        calendar::write_external_event_id,
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
        personality::evolve_personality,
        thoughts::list_thoughts,
        thoughts::get_thought,
        thoughts::update_thought,
        thoughts::generate_thoughts,
        integrations::list_integrations,
        integrations::create_integration,
        integrations::get_integration,
        integrations::delete_integration,
        audit::list_entries,
        chat::chat,
        signals::ingest_signal,
        sessions::list_sessions,
        sessions::get_session,
        sessions::delete_session,
    ),
    components(schemas(
        system::HealthResponse,
        auth::AuthResponse,
        users::UserResponse,
        users::UpdateUserRequest,
        settings::LlmSettings,
        settings::LlmSettingsPatch,
        settings::RolePromptPatch,
        settings::RolePromptResponse,
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
        calendar::SaveAppleEventRequest,
        calendar::WriteExternalEventRequest,
        permissions::PermissionSettings,
        permissions::PermissionPatch,
        project_members::AddMemberRequest,
        project_members::MemberResponse,
        task_dependencies::AddDependencyRequest,
        task_dependencies::DependencyResponse,
        personality::CreateSnapshotRequest,
        personality::SnapshotResponse,
        personality::EvolvePersonalityResponse,
        thoughts::UpdateThoughtRequest,
        thoughts::ThoughtResponse,
        thoughts::GenerateThoughtsResponse,
        integrations::IntegrationResponse,
        integrations::CreateIntegrationRequest,
        audit::AuditEntryResponse,
        chat::ChatRequest,
        signals::SignalIngestRequest,
        sessions::SessionResponse,
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
        // Unique path patterns: 34 unique URL patterns across all route groups.
        // system(2) + auth(1) + users(1) + settings(1) + calendar(3) +
        // projects(3) + project-health(1) + tasks(2) + conversations(2) +
        // chat(1) + sessions(2) + memories(3) + permissions(1) +
        // project_members(2) + task_dependencies(2) + personality(3) +
        // thoughts(2) + integrations(2) + audit(1) = 34 paths
        assert!(
            count >= 32,
            "Expected at least 32 registered path items, got {count}"
        );
    }

    /// CI contract gate: the committed `openapi.json` must match the live code.
    /// When this fails, run:
    ///
    /// ```bash
    /// curl -s http://localhost:8080/api-docs/openapi.json > openapi.json
    /// git add openapi.json
    /// ```
    #[test]
    fn openapi_spec_matches_committed_file() {
        let live = serde_json::to_value(openapi_spec()).expect("OpenApi should serialize");
        let committed_path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../openapi.json");
        let committed: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&committed_path).unwrap_or_default())
                .unwrap_or(serde_json::Value::Null);

        if live != committed {
            // Only fail if the committed file exists (CI has it, local may not)
            if committed_path.exists() {
                panic!(
                    "OpenAPI spec is out of date.\n\
                     Expected:  {}\n\
                     Run: curl -s http://localhost:8080/api-docs/openapi.json > openapi.json",
                    committed_path.display()
                );
            }
        }
    }
}
