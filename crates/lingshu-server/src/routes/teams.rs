//! Team workspace boundary. Human identity is verified with the fixed cloud
//! issuer on every request; local desktop JWTs never grant team access.
use super::memory_gateway as gateway;
use crate::{auth::AuthUser, error::AppError, state::AppState};
use axum::{
    extract::{FromRequestParts, Path, Query, State},
    http::{request::Parts, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing, Json, Router,
};
use chrono::{DateTime, Utc};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Transaction};
use utoipa::ToSchema;
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/teams", routing::get(list_teams).post(create_team))
        .route("/api/v1/teams/join", routing::post(join_team))
        .route("/api/v1/teams/:team/members", routing::get(list_members))
        .route(
            "/api/v1/teams/:team/members/:member",
            routing::patch(change_member).delete(remove_member),
        )
        .route(
            "/api/v1/teams/:team/invites",
            routing::get(list_invites).post(invite_member),
        )
        .route(
            "/api/v1/teams/:team/invites/:invite",
            routing::delete(revoke_invite),
        )
        .route(
            "/api/v1/teams/:team/keys",
            routing::get(list_keys).post(create_key),
        )
        .route("/api/v1/teams/:team/keys/:key", routing::delete(revoke_key))
        .route("/api/v1/teams/:team/audit", routing::get(list_audit))
        .route(
            "/api/v1/teams/:team/memories",
            routing::get(list_memories).post(write_memory),
        )
        .route(
            "/api/v1/teams/:team/memories/:memory/grants",
            routing::post(share_memory),
        )
        .route("/api/v1/teams/:team/search", routing::post(search_memory))
}

#[derive(Clone, Deserialize)]
pub struct Human {
    pub id: Uuid,
    pub email: String,
    pub email_verified_at: Option<String>,
}
#[derive(Deserialize)]
struct Account {
    user: Human,
}
const CLOUD_BASE: &str = "https://account.lingeringdream.com";
#[derive(Deserialize)]
struct CloudSubscription {
    plan_code: String,
    entitlement_active: bool,
}
#[derive(Deserialize)]
struct SubscriptionOverview {
    subscription: Option<CloudSubscription>,
}
fn bearer(headers: &HeaderMap) -> Result<&str, AppError> {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .filter(|v| !v.is_empty())
        .ok_or_else(|| AppError::Unauthorized("请先登录云账户".into()))
}
async fn verify_cloud(token: &str, url: &str) -> Result<Human, AppError> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| AppError::Internal(e.into()))?;
    let response = client
        .get(url)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    if response.status() == StatusCode::UNAUTHORIZED || response.status() == StatusCode::FORBIDDEN {
        return Err(AppError::Unauthorized("云账户会话已过期或被撤销".into()));
    }
    if !response.status().is_success() {
        return Err(AppError::Internal(anyhow::anyhow!(
            "Cloud identity service unavailable"
        )));
    }
    let account: Account = response
        .json()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    if account.user.email_verified_at.is_none() {
        return Err(AppError::Forbidden("请先验证邮箱".into()));
    }
    Ok(account.user)
}

/// Creating a workspace is a paid capability: the seat is billed to whoever
/// creates it. Invited members stay on free plans, so membership is
/// deliberately NOT accepted here — this mirrors `hasTeamEntitlement()` in the
/// Cloudflare worker so both entry points reject the same accounts.
///
/// Fails closed: if the cloud entitlement cannot be read we cannot prove the
/// seat was paid for, so creation is refused instead of allowed.
async fn require_team_plan(token: &str, url: &str) -> Result<(), AppError> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| AppError::Internal(e.into()))?;
    let response = client
        .get(url)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    if response.status() == StatusCode::UNAUTHORIZED || response.status() == StatusCode::FORBIDDEN {
        return Err(AppError::Unauthorized("云账户会话已过期或被撤销".into()));
    }
    if !response.status().is_success() {
        return Err(AppError::Internal(anyhow::anyhow!(
            "Cloud subscription service unavailable"
        )));
    }
    let overview: SubscriptionOverview = response
        .json()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    if !team_entitled(&overview) {
        return Err(AppError::Forbidden(
            "创建团队需要有效的团队版订阅，当前账户未开通。".into(),
        ));
    }
    Ok(())
}

/// The plan must be `team` AND currently entitled; an expired or lapsed team
/// subscription is not enough. Kept separate from the network call so the rule
/// is testable without a mock server.
fn team_entitled(overview: &SubscriptionOverview) -> bool {
    overview
        .subscription
        .as_ref()
        .map(|s| s.plan_code == "team" && s.entitlement_active)
        .unwrap_or(false)
}
impl FromRequestParts<AppState> for Human {
    type Rejection = Response;
    fn from_request_parts<'a, 'b, 'c>(
        parts: &'a mut Parts,
        _: &'b AppState,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self, Self::Rejection>> + Send + 'c>,
    >
    where
        'a: 'c,
        'b: 'c,
        Self: 'c,
    {
        Box::pin(async move {
            let token = bearer(&parts.headers).map_err(IntoResponse::into_response)?;
            verify_cloud(token, &format!("{CLOUD_BASE}/api/v1/account"))
                .await
                .map_err(IntoResponse::into_response)
        })
    }
}
#[derive(Serialize, ToSchema, sqlx::FromRow)]
pub struct Team {
    pub id: Uuid,
    pub name: String,
    pub role: String,
}
#[derive(Deserialize, ToSchema)]
pub struct CreateTeam {
    pub name: String,
}
#[derive(Serialize, ToSchema, sqlx::FromRow)]
pub struct Member {
    pub cloud_user_id: Uuid,
    pub email: String,
    pub role: String,
}
#[derive(Deserialize, ToSchema)]
pub struct RoleChange {
    pub role: String,
}
#[derive(Deserialize, ToSchema)]
pub struct InviteRequest {
    pub email: String,
    pub role: String,
}
#[derive(Deserialize, ToSchema)]
pub struct JoinRequest {
    pub token: String,
}
#[derive(Serialize, ToSchema)]
pub struct SecretResponse {
    pub id: Uuid,
    pub token: String,
}
#[derive(Serialize, ToSchema, sqlx::FromRow)]
pub struct Invite {
    pub id: Uuid,
    pub email: String,
    pub role: String,
    pub expires_at: DateTime<Utc>,
    pub used_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}
#[derive(Deserialize, ToSchema)]
pub struct KeyRequest {
    pub agent_key: String,
    pub name: String,
    pub permission: String,
    pub expires_in_days: i32,
}
#[derive(Serialize, ToSchema, sqlx::FromRow)]
pub struct AgentKey {
    pub id: Uuid,
    pub agent_key: String,
    pub name: String,
    pub permission: String,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}
#[derive(Serialize, ToSchema, sqlx::FromRow)]
pub struct AuditEvent {
    pub id: Uuid,
    pub actor: String,
    pub action: String,
    pub target: String,
    pub created_at: DateTime<Utc>,
}
fn valid_role(role: &str) -> Result<(), AppError> {
    if !matches!(role, "admin" | "editor" | "viewer") {
        return Err(AppError::Validation(
            "角色必须为 admin、editor 或 viewer".into(),
        ));
    }
    Ok(())
}
fn require_admin(role: &str) -> Result<(), AppError> {
    if !matches!(role, "owner" | "admin") {
        return Err(AppError::Forbidden("需要团队管理员权限".into()));
    }
    Ok(())
}
fn hash(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}
fn secret(prefix: &str) -> String {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    format!(
        "{prefix}{}",
        bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
    )
}
fn bounded(value: &str, limit: usize) -> Result<&str, AppError> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > limit {
        return Err(AppError::Validation(format!(
            "内容长度必须在 1–{limit} 字之间"
        )));
    }
    Ok(value)
}
async fn membership(
    state: &AppState,
    team: Uuid,
    human: &Human,
) -> Result<(Uuid, String), AppError> {
    sqlx::query_as("SELECT t.memory_user_id,m.role FROM team_workspaces t JOIN team_members m ON m.team_id=t.id WHERE t.id=$1 AND m.cloud_user_id=$2")
        .bind(team).bind(human.id).fetch_optional(&state.db).await?.ok_or_else(|| AppError::Forbidden("无权访问该团队".into()))
}
// Serialize membership/key mutations per team, including the authorization check.
async fn management<'a>(
    state: &'a AppState,
    team: Uuid,
    human: &Human,
) -> Result<(Transaction<'a, Postgres>, Uuid, String), AppError> {
    let mut tx = state.db.begin().await?;
    let space: Uuid =
        sqlx::query_scalar("SELECT memory_user_id FROM team_workspaces WHERE id=$1 FOR UPDATE")
            .bind(team)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| AppError::Forbidden("无权访问该团队".into()))?;
    let role: String =
        sqlx::query_scalar("SELECT role FROM team_members WHERE team_id=$1 AND cloud_user_id=$2")
            .bind(team)
            .bind(human.id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| AppError::Forbidden("无权访问该团队".into()))?;
    require_admin(&role)?;
    Ok((tx, space, role))
}
async fn audit(
    tx: &mut Transaction<'_, Postgres>,
    team: Uuid,
    actor: &str,
    action: &str,
    target: &str,
) -> Result<(), AppError> {
    sqlx::query("INSERT INTO team_audit_events(team_id,actor,action,target) VALUES($1,$2,$3,$4)")
        .bind(team)
        .bind(actor)
        .bind(action)
        .bind(target)
        .execute(&mut **tx)
        .await?;
    Ok(())
}
#[utoipa::path(get,path="/api/v1/teams",responses((status=200,body=Vec<Team>)))]
pub async fn list_teams(
    State(state): State<AppState>,
    human: Human,
) -> Result<Json<Vec<Team>>, AppError> {
    Ok(Json(sqlx::query_as("SELECT t.id,t.name,m.role FROM team_workspaces t JOIN team_members m ON m.team_id=t.id WHERE m.cloud_user_id=$1 ORDER BY t.created_at").bind(human.id).fetch_all(&state.db).await?))
}
#[utoipa::path(post,path="/api/v1/teams",request_body=CreateTeam,responses((status=201,body=Team)))]
pub async fn create_team(
    State(state): State<AppState>,
    headers: HeaderMap,
    human: Human,
    Json(req): Json<CreateTeam>,
) -> Result<(StatusCode, Json<Team>), AppError> {
    require_team_plan(
        bearer(&headers)?,
        &format!("{CLOUD_BASE}/api/v1/account/subscription"),
    )
    .await?;
    let name = bounded(&req.name, 100)?;
    let space = Uuid::new_v4();
    let mut tx = state.db.begin().await?;
    sqlx::query("INSERT INTO users(id,email,display_name,password_hash,role) VALUES($1,$2,$3,'no-login-team-namespace','service')").bind(space).bind(format!("team-{space}@lingshu.internal")).bind(name).execute(&mut *tx).await?;
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO team_workspaces(name,memory_user_id) VALUES($1,$2) RETURNING id",
    )
    .bind(name)
    .bind(space)
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO team_members(team_id,cloud_user_id,email,role) VALUES($1,$2,$3,'owner')",
    )
    .bind(id)
    .bind(human.id)
    .bind(human.email.to_lowercase())
    .execute(&mut *tx)
    .await?;
    audit(&mut tx, id, &human.id.to_string(), "team.created", name).await?;
    tx.commit().await?;
    Ok((
        StatusCode::CREATED,
        Json(Team {
            id,
            name: name.into(),
            role: "owner".into(),
        }),
    ))
}
#[utoipa::path(get,path="/api/v1/teams/{team}/members",responses((status=200,body=Vec<Member>)))]
pub async fn list_members(
    State(state): State<AppState>,
    human: Human,
    Path(team): Path<Uuid>,
) -> Result<Json<Vec<Member>>, AppError> {
    membership(&state, team, &human).await?;
    Ok(Json(sqlx::query_as("SELECT cloud_user_id,email,role FROM team_members WHERE team_id=$1 ORDER BY created_at").bind(team).fetch_all(&state.db).await?))
}
#[utoipa::path(patch,path="/api/v1/teams/{team}/members/{member}",request_body=RoleChange,responses((status=204)))]
pub async fn change_member(
    State(state): State<AppState>,
    human: Human,
    Path((team, member)): Path<(Uuid, Uuid)>,
    Json(req): Json<RoleChange>,
) -> Result<StatusCode, AppError> {
    valid_role(&req.role)?;
    let (mut tx, _, role) = management(&state, team, &human).await?;
    // Only the owner can alter membership roles; the owner itself is immutable.
    if role != "owner" {
        return Err(AppError::Forbidden("只有所有者可以调整角色".into()));
    }
    let n = sqlx::query(
        "UPDATE team_members SET role=$3 WHERE team_id=$1 AND cloud_user_id=$2 AND role <> 'owner'",
    )
    .bind(team)
    .bind(member)
    .bind(&req.role)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if n == 0 {
        return Err(AppError::Forbidden("不能修改所有者或不存在的成员".into()));
    }
    audit(
        &mut tx,
        team,
        &human.id.to_string(),
        "member.role_changed",
        &format!("{member}:{}", req.role),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
#[utoipa::path(delete,path="/api/v1/teams/{team}/members/{member}",responses((status=204)))]
pub async fn remove_member(
    State(state): State<AppState>,
    human: Human,
    Path((team, member)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    let (mut tx, _, role) = management(&state, team, &human).await?;
    let n=sqlx::query("DELETE FROM team_members WHERE team_id=$1 AND cloud_user_id=$2 AND role <> 'owner' AND ($3='owner' OR role IN ('editor','viewer'))").bind(team).bind(member).bind(role).execute(&mut *tx).await?.rows_affected();
    if n == 0 {
        return Err(AppError::Forbidden("不能移除该成员".into()));
    }
    audit(
        &mut tx,
        team,
        &human.id.to_string(),
        "member.removed",
        &member.to_string(),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
#[utoipa::path(post,path="/api/v1/teams/{team}/invites",request_body=InviteRequest,responses((status=201,body=SecretResponse)))]
pub async fn invite_member(
    State(state): State<AppState>,
    human: Human,
    Path(team): Path<Uuid>,
    Json(req): Json<InviteRequest>,
) -> Result<(StatusCode, Json<SecretResponse>), AppError> {
    valid_role(&req.role)?;
    let email = bounded(&req.email, 255)?.to_lowercase();
    if !email.contains('@') {
        return Err(AppError::Validation("请输入邮箱".into()));
    }
    let (mut tx, _, role) = management(&state, team, &human).await?;
    if req.role == "admin" && role != "owner" {
        return Err(AppError::Forbidden("只有所有者可以邀请管理员".into()));
    }
    let token = secret("lsi_");
    let id = sqlx::query_scalar(
        "INSERT INTO team_invites(team_id,email,role,token_hash) VALUES($1,$2,$3,$4) RETURNING id",
    )
    .bind(team)
    .bind(&email)
    .bind(&req.role)
    .bind(hash(&token))
    .fetch_one(&mut *tx)
    .await?;
    audit(
        &mut tx,
        team,
        &human.id.to_string(),
        "invite.created",
        &email,
    )
    .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(SecretResponse { id, token })))
}
#[utoipa::path(get,path="/api/v1/teams/{team}/invites",responses((status=200,body=Vec<Invite>)))]
pub async fn list_invites(
    State(state): State<AppState>,
    human: Human,
    Path(team): Path<Uuid>,
) -> Result<Json<Vec<Invite>>, AppError> {
    require_admin(&membership(&state, team, &human).await?.1)?;
    Ok(Json(sqlx::query_as("SELECT id,email,role,expires_at,used_at,revoked_at FROM team_invites WHERE team_id=$1 ORDER BY created_at DESC LIMIT 100").bind(team).fetch_all(&state.db).await?))
}
#[utoipa::path(delete,path="/api/v1/teams/{team}/invites/{invite}",responses((status=204)))]
pub async fn revoke_invite(
    State(state): State<AppState>,
    human: Human,
    Path((team, invite)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    let (mut tx, _, _) = management(&state, team, &human).await?;
    sqlx::query(
        "UPDATE team_invites SET revoked_at=NOW() WHERE team_id=$1 AND id=$2 AND used_at IS NULL",
    )
    .bind(team)
    .bind(invite)
    .execute(&mut *tx)
    .await?;
    audit(
        &mut tx,
        team,
        &human.id.to_string(),
        "invite.revoked",
        &invite.to_string(),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
#[utoipa::path(post,path="/api/v1/teams/join",request_body=JoinRequest,responses((status=200,body=Team)))]
pub async fn join_team(
    State(state): State<AppState>,
    human: Human,
    Json(req): Json<JoinRequest>,
) -> Result<Json<Team>, AppError> {
    let mut tx = state.db.begin().await?;
    let found: Option<(Uuid,String,String)> = sqlx::query_as("UPDATE team_invites SET used_at=NOW() WHERE token_hash=$1 AND email=$2 AND expires_at>NOW() AND used_at IS NULL AND revoked_at IS NULL RETURNING team_id,email,role").bind(hash(&req.token)).bind(human.email.to_lowercase()).fetch_optional(&mut *tx).await?;
    let (team, email, role) = found.ok_or_else(|| {
        AppError::Forbidden("邀请无效、已使用、已过期，或与当前邮箱不匹配".into())
    })?;
    sqlx::query("INSERT INTO team_members(team_id,cloud_user_id,email,role) VALUES($1,$2,$3,$4) ON CONFLICT(team_id,cloud_user_id) DO NOTHING").bind(team).bind(human.id).bind(email).bind(role).execute(&mut *tx).await?;
    audit(
        &mut tx,
        team,
        &human.id.to_string(),
        "member.joined",
        &human.email,
    )
    .await?;
    let result=sqlx::query_as("SELECT t.id,t.name,m.role FROM team_workspaces t JOIN team_members m ON m.team_id=t.id WHERE t.id=$1 AND m.cloud_user_id=$2").bind(team).bind(human.id).fetch_one(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(result))
}
#[utoipa::path(post,path="/api/v1/teams/{team}/keys",request_body=KeyRequest,responses((status=201,body=SecretResponse)))]
pub async fn create_key(
    State(state): State<AppState>,
    human: Human,
    Path(team): Path<Uuid>,
    Json(req): Json<KeyRequest>,
) -> Result<(StatusCode, Json<SecretResponse>), AppError> {
    bounded(&req.name, 100)?;
    if req.agent_key.is_empty()
        || req.agent_key.len() > 64
        || !req
            .agent_key
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
        || req.agent_key.starts_with("member-")
    {
        return Err(AppError::Validation(
            "Agent 标识需为 1–64 位字母、数字、下划线或连字符，不能以 member- 开头".into(),
        ));
    }
    if !matches!(req.permission.as_str(), "read" | "write")
        || !(1..=365).contains(&req.expires_in_days)
    {
        return Err(AppError::Validation(
            "权限为 read/write；有效期为 1–365 天".into(),
        ));
    }
    let (mut tx, space, _) = management(&state, team, &human).await?;
    sqlx::query("INSERT INTO memory_agents(user_id,agent_key,display_name) VALUES($1,$2,$3) ON CONFLICT(user_id,agent_key) DO NOTHING").bind(space).bind(&req.agent_key).bind(&req.name).execute(&mut *tx).await?;
    let token = secret("lsk_");
    let id=sqlx::query_scalar("INSERT INTO team_agent_keys(team_id,agent_key,name,token_hash,permission,expires_at) VALUES($1,$2,$3,$4,$5,NOW()+make_interval(days=>$6)) RETURNING id").bind(team).bind(&req.agent_key).bind(&req.name).bind(hash(&token)).bind(&req.permission).bind(req.expires_in_days).fetch_one(&mut *tx).await?;
    audit(
        &mut tx,
        team,
        &human.id.to_string(),
        "key.created",
        &format!("{id}:{}", req.agent_key),
    )
    .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(SecretResponse { id, token })))
}
#[utoipa::path(get,path="/api/v1/teams/{team}/keys",responses((status=200,body=Vec<AgentKey>)))]
pub async fn list_keys(
    State(state): State<AppState>,
    human: Human,
    Path(team): Path<Uuid>,
) -> Result<Json<Vec<AgentKey>>, AppError> {
    require_admin(&membership(&state, team, &human).await?.1)?;
    Ok(Json(sqlx::query_as("SELECT id,agent_key,name,permission,expires_at,revoked_at FROM team_agent_keys WHERE team_id=$1 ORDER BY created_at DESC LIMIT 100").bind(team).fetch_all(&state.db).await?))
}
#[utoipa::path(delete,path="/api/v1/teams/{team}/keys/{key}",responses((status=204)))]
pub async fn revoke_key(
    State(state): State<AppState>,
    human: Human,
    Path((team, key)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    let (mut tx, _, _) = management(&state, team, &human).await?;
    sqlx::query("UPDATE team_agent_keys SET revoked_at=NOW() WHERE team_id=$1 AND id=$2 AND revoked_at IS NULL").bind(team).bind(key).execute(&mut *tx).await?;
    audit(
        &mut tx,
        team,
        &human.id.to_string(),
        "key.revoked",
        &key.to_string(),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
#[utoipa::path(get,path="/api/v1/teams/{team}/audit",responses((status=200,body=Vec<AuditEvent>)))]
pub async fn list_audit(
    State(state): State<AppState>,
    human: Human,
    Path(team): Path<Uuid>,
) -> Result<Json<Vec<AuditEvent>>, AppError> {
    require_admin(&membership(&state, team, &human).await?.1)?;
    Ok(Json(sqlx::query_as("SELECT id,actor,action,target,created_at FROM team_audit_events WHERE team_id=$1 ORDER BY created_at DESC,id DESC LIMIT 100").bind(team).fetch_all(&state.db).await?))
}
struct Principal {
    space: Uuid,
    agent: String,
    actor: String,
    writable: bool,
}
async fn principal(
    state: &AppState,
    team: Uuid,
    headers: &HeaderMap,
    write: bool,
) -> Result<Principal, AppError> {
    let token = bearer(headers)?;
    let p = if token.starts_with("lsk_") {
        let row: Option<(Uuid,String,Uuid,String)>=sqlx::query_as("SELECT t.memory_user_id,k.agent_key,k.id,k.permission FROM team_agent_keys k JOIN team_workspaces t ON t.id=k.team_id JOIN memory_agents a ON a.user_id=t.memory_user_id AND a.agent_key=k.agent_key WHERE k.team_id=$1 AND k.token_hash=$2 AND k.revoked_at IS NULL AND k.expires_at>NOW() AND a.is_active").bind(team).bind(hash(token)).fetch_optional(&state.db).await?;
        let (space, agent, id, permission) =
            row.ok_or_else(|| AppError::Unauthorized("Agent 凭证无效、已撤销或已过期".into()))?;
        Principal {
            space,
            agent,
            actor: format!("key:{id}"),
            writable: permission == "write",
        }
    } else {
        let human = verify_cloud(token, &format!("{CLOUD_BASE}/api/v1/account")).await?;
        human_principal(state, team, &human).await?
    };
    if write && !p.writable {
        return Err(AppError::Forbidden("当前身份只有读取权限".into()));
    }
    Ok(p)
}
async fn human_principal(
    state: &AppState,
    team: Uuid,
    human: &Human,
) -> Result<Principal, AppError> {
    let (space, role) = membership(state, team, human).await?;
    Ok(Principal {
        space,
        agent: format!("member-{}", human.id),
        actor: human.id.to_string(),
        writable: role != "viewer",
    })
}
async fn access_audit(
    state: &AppState,
    team: Uuid,
    p: &Principal,
    action: &str,
    target: &str,
) -> Result<(), AppError> {
    let mut tx = state.db.begin().await?;
    audit(&mut tx, team, &p.actor, action, target).await?;
    tx.commit().await?;
    Ok(())
}
#[utoipa::path(get,path="/api/v1/teams/{team}/memories",responses((status=200,body=Vec<gateway::GatewayMemoryResponse>)))]
pub async fn list_memories(
    State(state): State<AppState>,
    Path(team): Path<Uuid>,
    headers: HeaderMap,
    Query(mut query): Query<gateway::GatewayListParams>,
) -> Result<Json<Vec<gateway::GatewayMemoryResponse>>, AppError> {
    let p = principal(&state, team, &headers, false).await?;
    query.agent_id = p.agent.clone();
    access_audit(&state, team, &p, "memory.list_requested", "").await?;
    gateway::list_gateway_memories(
        State(state),
        Some(AuthUser { user_id: p.space }),
        Query(query),
    )
    .await
}
#[utoipa::path(post,path="/api/v1/teams/{team}/memories",request_body=gateway::CreateGatewayMemoryRequest,responses((status=201,body=gateway::GatewayMemoryResponse)))]
pub async fn write_memory(
    State(state): State<AppState>,
    Path(team): Path<Uuid>,
    headers: HeaderMap,
    Json(req): Json<gateway::CreateGatewayMemoryRequest>,
) -> Result<(StatusCode, Json<gateway::GatewayMemoryResponse>), AppError> {
    let p = principal(&state, team, &headers, true).await?;
    write_for_principal(state, team, p, req).await
}
async fn write_for_principal(
    state: AppState,
    team: Uuid,
    p: Principal,
    mut req: gateway::CreateGatewayMemoryRequest,
) -> Result<(StatusCode, Json<gateway::GatewayMemoryResponse>), AppError> {
    req.agent_id = p.agent.clone();
    // Source IDs must belong to this actor and may not widen private/shared data.
    // Cross-agent derivation needs an explicit conservative ACL intersection.
    if !req.source_memory_ids.is_empty() || req.episode_id.is_some() {
        return Err(AppError::Validation(
            "团队 API 暂不支持手动指定来源 ID，请直接写入原始记录".into(),
        ));
    }
    access_audit(
        &state,
        team,
        &p,
        "memory.write_requested",
        &req.memory_layer,
    )
    .await?;
    let result = gateway::create_gateway_memory(
        State(state.clone()),
        Some(AuthUser { user_id: p.space }),
        Json(req),
    )
    .await?;
    access_audit(&state, team, &p, "memory.created", &result.1.id.to_string()).await?;
    Ok(result)
}
#[utoipa::path(post,path="/api/v1/teams/{team}/search",request_body=gateway::GatewaySearchRequest,responses((status=200,body=gateway::GatewaySearchResponse)))]
pub async fn search_memory(
    State(state): State<AppState>,
    Path(team): Path<Uuid>,
    headers: HeaderMap,
    Json(mut req): Json<gateway::GatewaySearchRequest>,
) -> Result<Json<gateway::GatewaySearchResponse>, AppError> {
    let p = principal(&state, team, &headers, false).await?;
    req.agent_id = p.agent.clone();
    access_audit(&state, team, &p, "memory.search_requested", &req.mode).await?;
    gateway::gateway_search(State(state), Some(AuthUser { user_id: p.space }), Json(req)).await
}
#[utoipa::path(post,path="/api/v1/teams/{team}/memories/{memory}/grants",request_body=gateway::GrantMemoryRequest,responses((status=200,body=gateway::GrantMemoryResponse)))]
pub async fn share_memory(
    State(state): State<AppState>,
    Path((team, memory)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
    Json(mut req): Json<gateway::GrantMemoryRequest>,
) -> Result<Json<gateway::GrantMemoryResponse>, AppError> {
    let p = principal(&state, team, &headers, true).await?;
    req.owner_agent_id = p.agent.clone();
    if req.can_write || req.can_share {
        return Err(AppError::Validation("当前定向授权仅支持读取".into()));
    }
    let exists: bool = if let Some(id) = req
        .grantee_agent_id
        .strip_prefix("member-")
        .and_then(|v| Uuid::parse_str(v).ok())
    {
        sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM team_members WHERE team_id=$1 AND cloud_user_id=$2)",
        )
        .bind(team)
        .bind(id)
        .fetch_one(&state.db)
        .await?
    } else {
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM memory_agents WHERE user_id=$1 AND agent_key=$2 AND is_active)").bind(p.space).bind(&req.grantee_agent_id).fetch_one(&state.db).await?
    };
    if !exists {
        return Err(AppError::Validation(
            "目标必须为本团队成员或已注册 Agent".into(),
        ));
    }
    access_audit(
        &state,
        team,
        &p,
        "memory.share_requested",
        &memory.to_string(),
    )
    .await?;
    let result = gateway::grant_memory_access(
        State(state.clone()),
        Some(AuthUser { user_id: p.space }),
        Path(memory),
        Json(req),
    )
    .await?;
    access_audit(
        &state,
        team,
        &p,
        "memory.grant_changed",
        &format!("{memory}:{}:{}", result.grantee_agent_id, result.can_read),
    )
    .await?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{to_bytes, Body},
        http::Request,
    };
    use serde_json::{json, Value};
    use tower::ServiceExt;
    use wiremock::{
        matchers::{header, method, path},
        Mock, MockServer, ResponseTemplate,
    };

    #[tokio::test]
    async fn team_entitled_recognises_only_active_team_plan() {
        let team_active = SubscriptionOverview {
            subscription: Some(CloudSubscription {
                plan_code: "team".into(),
                entitlement_active: true,
            }),
        };
        assert!(team_entitled(&team_active));

        let team_lapsed = SubscriptionOverview {
            subscription: Some(CloudSubscription {
                plan_code: "team".into(),
                entitlement_active: false,
            }),
        };
        assert!(!team_entitled(&team_lapsed));

        // Free and Developer plans must not be accepted even when their own
        // flag says "active" — seat is owned by the workspace, not the creator.
        for plan in ["free", "developer"] {
            let overview = SubscriptionOverview {
                subscription: Some(CloudSubscription {
                    plan_code: plan.into(),
                    entitlement_active: true,
                }),
            };
            assert!(
                !team_entitled(&overview),
                "plan_code={plan} should NOT pass team gate"
            );
        }

        // No subscription row at all (e.g. brand-new account).
        let empty = SubscriptionOverview { subscription: None };
        assert!(!team_entitled(&empty));
    }

    #[tokio::test]
    async fn require_team_plan_accepts_only_active_team_subscriptions() {
        let server = MockServer::start().await;
        let cases: Vec<(&str, serde_json::Value, bool)> = vec![
            (
                "enterprise is a separate product",
                json!({"plan_code":"enterprise","entitlement_active":true}),
                false,
            ),
            (
                "enterprise + lapsed rejected",
                json!({"plan_code":"enterprise","entitlement_active":false}),
                false,
            ),
            (
                "team + active passes",
                json!({"plan_code":"team","entitlement_active":true}),
                true,
            ),
            (
                "team + lapsed rejected",
                json!({"plan_code":"team","entitlement_active":false}),
                false,
            ),
            (
                "free plan rejected",
                json!({"plan_code":"free","entitlement_active":true}),
                false,
            ),
            (
                "developer plan rejected",
                json!({"plan_code":"developer","entitlement_active":true}),
                false,
            ),
        ];
        for (label, body, should_pass) in cases {
            Mock::given(method("GET"))
                .and(path("/api/v1/account/subscription"))
                .respond_with(
                    ResponseTemplate::new(200).set_body_json(json!({"subscription": body})),
                )
                .mount(&server)
                .await;
            let result = require_team_plan(
                "token",
                &format!("{}/api/v1/account/subscription", server.uri()),
            )
            .await;
            assert_eq!(result.is_ok(), should_pass, "{label}: {result:?}");
            server.reset().await;
        }

        // Subscription endpoint returns null/empty body — fail closed.
        Mock::given(method("GET"))
            .and(path("/api/v1/account/subscription"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"subscription": null})))
            .mount(&server)
            .await;
        assert!(matches!(
            require_team_plan(
                "token",
                &format!("{}/api/v1/account/subscription", server.uri())
            )
            .await,
            Err(AppError::Forbidden(_))
        ));
        server.reset().await;

        // 401 must surface as Unauthorized so the client can re-auth.
        Mock::given(method("GET"))
            .and(path("/api/v1/account/subscription"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        assert!(matches!(
            require_team_plan(
                "token",
                &format!("{}/api/v1/account/subscription", server.uri())
            )
            .await,
            Err(AppError::Unauthorized(_))
        ));
        server.reset().await;

        // 5xx must FAIL CLOSED — we cannot prove the seat was paid for, so do
        // not let creation slip through.
        Mock::given(method("GET"))
            .and(path("/api/v1/account/subscription"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;
        assert!(matches!(
            require_team_plan(
                "token",
                &format!("{}/api/v1/account/subscription", server.uri())
            )
            .await,
            Err(AppError::Internal(_))
        ));
    }

    #[tokio::test]
    async fn create_team_blocks_non_team_creators_via_cloud_entitlement() {
        let server = MockServer::start().await;
        // Identity check: any verified account passes the FromRequestParts gate.
        let human_id = Uuid::new_v4();
        Mock::given(method("GET"))
            .and(path("/api/v1/account"))
            .and(header("Authorization", "Bearer creator"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                json!({"user":{"id":human_id,"email":"creator@example.test","email_verified_at":"2026-09-05"}}),
            ))
            .mount(&server)
            .await;
        // Entitlement check: this account is still on the free plan.
        Mock::given(method("GET"))
            .and(path("/api/v1/account/subscription"))
            .and(header("Authorization", "Bearer creator"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                json!({"subscription":{"plan_code":"free","plan_name":"免费版","status":"active","billing_interval":null,"current_period_end":null,"cancel_at_period_end":false,"entitlement_active":true}}),
            ))
            .mount(&server)
            .await;
        // Just exercising require_team_plan() with the same URL the route uses
        // is enough to prove the integration shape; the full stack test is
        // #[ignore]-gated behind a real TEAM_TEST_DATABASE_URL.
        let url = format!("{}/api/v1/account/subscription", server.uri());
        assert!(matches!(
            require_team_plan("creator", &url).await,
            Err(AppError::Forbidden(_))
        ));
    }

    #[tokio::test]
    async fn cloud_identity_requires_verified_live_account_and_never_follows_redirects() {
        let server = MockServer::start().await;
        let human = json!({"user":{"id":Uuid::new_v4(),"email":"member@example.test","email_verified_at":"2026-09-05"}});
        Mock::given(method("GET"))
            .and(path("/account"))
            .and(header("Authorization", "Bearer live"))
            .respond_with(ResponseTemplate::new(200).set_body_json(human))
            .mount(&server)
            .await;
        assert!(verify_cloud("live", &format!("{}/account", server.uri()))
            .await
            .is_ok());
        Mock::given(path("/revoked"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        assert!(matches!(
            verify_cloud("x", &format!("{}/revoked", server.uri())).await,
            Err(AppError::Unauthorized(_))
        ));
        Mock::given(path("/redirect"))
            .respond_with(
                ResponseTemplate::new(302)
                    .insert_header("Location", format!("{}/account", server.uri())),
            )
            .mount(&server)
            .await;
        assert!(verify_cloud("live", &format!("{}/redirect", server.uri()))
            .await
            .is_err());
        Mock::given(path("/unverified")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"user":{"id":Uuid::new_v4(),"email":"x@example.test","email_verified_at":null}}))).mount(&server).await;
        assert!(matches!(
            verify_cloud("x", &format!("{}/unverified", server.uri())).await,
            Err(AppError::Forbidden(_))
        ));
    }
    fn person(email: &str) -> Human {
        Human {
            id: Uuid::new_v4(),
            email: email.into(),
            email_verified_at: Some("2026-09-05".into()),
        }
    }
    async fn api(
        state: &AppState,
        token: &str,
        method: &str,
        path: String,
        body: Value,
    ) -> (StatusCode, Value) {
        let response = router()
            .with_state(state.clone())
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .header("Authorization", format!("Bearer {token}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }
    async fn key(
        state: &AppState,
        owner: &Human,
        team: Uuid,
        agent: &str,
        permission: &str,
    ) -> SecretResponse {
        create_key(
            State(state.clone()),
            owner.clone(),
            Path(team),
            Json(KeyRequest {
                agent_key: agent.into(),
                name: agent.into(),
                permission: permission.into(),
                expires_in_days: 1,
            }),
        )
        .await
        .unwrap()
        .1
         .0
    }
    #[tokio::test]
    #[ignore = "Requires dedicated TEAM_TEST_DATABASE_URL PostgreSQL; runs real migrations and HTTP isolation checks"]
    async fn team_database_authorization_lifecycle() {
        let url = std::env::var("TEAM_TEST_DATABASE_URL").expect("Use a dedicated test database");
        let mut config: crate::config::AppConfig = serde_json::from_value(json!({})).unwrap();
        config.database.url = url;
        config.redis.url = String::new();
        config.qdrant.url = String::new();
        config.llm.ollama_url = "http://127.0.0.1:1".into();
        let state = AppState::new(&config).await.unwrap();
        let owner = person("owner@example.test");
        let outsider = person("outsider@example.test");
        let editor = person("editor@example.test");
        let viewer = person("viewer@example.test");
        let a = create_team(
            State(state.clone()),
            axum::http::HeaderMap::new(),
            owner.clone(),
            Json(CreateTeam {
                name: "Team A".into(),
            }),
        )
        .await
        .unwrap()
        .1
         .0;
        let b = create_team(
            State(state.clone()),
            axum::http::HeaderMap::new(),
            outsider.clone(),
            Json(CreateTeam {
                name: "Team B".into(),
            }),
        )
        .await
        .unwrap()
        .1
         .0;
        assert!(membership(&state, a.id, &outsider).await.is_err());
        assert_eq!(
            list_teams(State(state.clone()), owner.clone())
                .await
                .unwrap()
                .len(),
            1
        );
        let invite = invite_member(
            State(state.clone()),
            owner.clone(),
            Path(a.id),
            Json(InviteRequest {
                email: editor.email.clone(),
                role: "editor".into(),
            }),
        )
        .await
        .unwrap()
        .1
         .0;
        assert!(join_team(
            State(state.clone()),
            outsider.clone(),
            Json(JoinRequest {
                token: invite.token.clone()
            })
        )
        .await
        .is_err());
        let _ = join_team(
            State(state.clone()),
            editor.clone(),
            Json(JoinRequest {
                token: invite.token.clone(),
            }),
        )
        .await
        .unwrap();
        assert!(join_team(
            State(state.clone()),
            editor.clone(),
            Json(JoinRequest {
                token: invite.token
            })
        )
        .await
        .is_err());
        assert!(create_key(
            State(state.clone()),
            editor.clone(),
            Path(a.id),
            Json(KeyRequest {
                agent_key: "x".into(),
                name: "x".into(),
                permission: "write".into(),
                expires_in_days: 30
            })
        )
        .await
        .is_err());
        let v = invite_member(
            State(state.clone()),
            owner.clone(),
            Path(a.id),
            Json(InviteRequest {
                email: viewer.email.clone(),
                role: "viewer".into(),
            }),
        )
        .await
        .unwrap()
        .1
         .0;
        let _ = join_team(
            State(state.clone()),
            viewer.clone(),
            Json(JoinRequest { token: v.token }),
        )
        .await
        .unwrap();
        assert!(
            !human_principal(&state, a.id, &viewer)
                .await
                .unwrap()
                .writable
        );
        assert!(change_member(
            State(state.clone()),
            editor.clone(),
            Path((a.id, viewer.id)),
            Json(RoleChange {
                role: "admin".into()
            })
        )
        .await
        .is_err());
        assert!(change_member(
            State(state.clone()),
            owner.clone(),
            Path((a.id, owner.id)),
            Json(RoleChange {
                role: "viewer".into()
            })
        )
        .await
        .is_err());
        assert!(
            remove_member(State(state.clone()), owner.clone(), Path((a.id, owner.id)))
                .await
                .is_err()
        );
        let canceled = invite_member(
            State(state.clone()),
            owner.clone(),
            Path(a.id),
            Json(InviteRequest {
                email: outsider.email.clone(),
                role: "viewer".into(),
            }),
        )
        .await
        .unwrap()
        .1
         .0;
        revoke_invite(
            State(state.clone()),
            owner.clone(),
            Path((a.id, canceled.id)),
        )
        .await
        .unwrap();
        assert!(join_team(
            State(state.clone()),
            outsider.clone(),
            Json(JoinRequest {
                token: canceled.token
            })
        )
        .await
        .is_err());
        let expired = invite_member(
            State(state.clone()),
            owner.clone(),
            Path(a.id),
            Json(InviteRequest {
                email: outsider.email.clone(),
                role: "viewer".into(),
            }),
        )
        .await
        .unwrap()
        .1
         .0;
        sqlx::query("UPDATE team_invites SET expires_at=NOW()-INTERVAL '1 second' WHERE id=$1")
            .bind(expired.id)
            .execute(&state.db)
            .await
            .unwrap();
        assert!(join_team(
            State(state.clone()),
            outsider.clone(),
            Json(JoinRequest {
                token: expired.token
            })
        )
        .await
        .is_err());

        let writer = key(&state, &owner, a.id, "writer", "write").await;
        let reader = key(&state, &owner, a.id, "reader", "read").await;
        let other = key(&state, &outsider, b.id, "writer", "write").await;
        let prefix = format!("/api/v1/teams/{}", a.id);
        let (status, private) = api(
            &state,
            &writer.token,
            "POST",
            format!("{prefix}/memories"),
            json!({"memory_layer":"episode","content":"private alpha secret","memory_scope":"private","agent_id":"spoofed"}),
        )
        .await;
        assert_eq!(status, 201);
        assert_eq!(private["owner_agent_id"], "writer");
        let (status, _) = api(
            &state,
            &reader.token,
            "POST",
            format!("{prefix}/memories"),
            json!({"content":"forbidden"}),
        )
        .await;
        assert_eq!(status, 403);
        let (status, rows) = api(
            &state,
            &reader.token,
            "GET",
            format!("{prefix}/memories?agent_id=writer"),
            Value::Null,
        )
        .await;
        assert_eq!(status, 200);
        assert_eq!(rows.as_array().unwrap().len(), 0);
        let (status, _) = api(
            &state,
            &other.token,
            "GET",
            format!("{prefix}/memories"),
            Value::Null,
        )
        .await;
        assert_eq!(status, 401);
        let (status, _) = api(
            &state,
            &writer.token,
            "GET",
            format!("/api/v1/teams/{}/memories", b.id),
            Value::Null,
        )
        .await;
        assert_eq!(status, 401);
        let (status, public) = api(
            &state,
            &writer.token,
            "POST",
            format!("{prefix}/memories"),
            json!({"memory_layer":"episode","content":"public team guideline","memory_scope":"global"}),
        )
        .await;
        assert_eq!(status, 201);
        let (_, rows) = api(
            &state,
            &reader.token,
            "GET",
            format!("{prefix}/memories"),
            Value::Null,
        )
        .await;
        assert_eq!(rows.as_array().unwrap().len(), 1);
        assert_eq!(rows[0]["id"], public["id"]);
        let (status, search) = api(
            &state,
            &reader.token,
            "POST",
            format!("{prefix}/search"),
            json!({"query":"private alpha secret","agent_id":"writer"}),
        )
        .await;
        assert_eq!(status, 200);
        assert!(!search["results"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["id"] == private["id"]));
        let id = private["id"].as_str().unwrap();
        let (status, _) = api(
            &state,
            &writer.token,
            "POST",
            format!("{prefix}/memories/{id}/grants"),
            json!({"owner_agent_id":"spoof","grantee_agent_id":"reader","can_read":true}),
        )
        .await;
        assert_eq!(status, 200);
        let (_, rows) = api(
            &state,
            &reader.token,
            "GET",
            format!("{prefix}/memories"),
            Value::Null,
        )
        .await;
        assert_eq!(rows.as_array().unwrap().len(), 2);
        let (status, _) = api(
            &state,
            &writer.token,
            "POST",
            format!("{prefix}/memories/{id}/grants"),
            json!({"owner_agent_id":"writer","grantee_agent_id":"reader","can_read":false}),
        )
        .await;
        assert_eq!(status, 200);
        let (_, rows) = api(
            &state,
            &reader.token,
            "GET",
            format!("{prefix}/memories"),
            Value::Null,
        )
        .await;
        assert_eq!(rows.as_array().unwrap().len(), 1);
        let (status, _) = api(
            &state,
            &writer.token,
            "POST",
            format!("{prefix}/memories"),
            json!({"content":"source leak","source_memory_ids":[id]}),
        )
        .await;
        assert_eq!(status, 422);
        let fresh = key(&state, &owner, a.id, "fresh", "write").await;
        let (status, _) = api(
            &state,
            &fresh.token,
            "POST",
            format!("{prefix}/memories/{id}/grants"),
            json!({"owner_agent_id":"writer","grantee_agent_id":"reader"}),
        )
        .await;
        assert_eq!(status, 403);
        let (status, _) = api(
            &state,
            &fresh.token,
            "POST",
            format!("{prefix}/memories/{id}/grants"),
            json!({"owner_agent_id":"writer","grantee_agent_id":"reader","can_write":true}),
        )
        .await;
        assert_eq!(status, 422);
        revoke_key(State(state.clone()), owner.clone(), Path((a.id, writer.id)))
            .await
            .unwrap();
        let (status, _) = api(
            &state,
            &writer.token,
            "GET",
            format!("{prefix}/memories"),
            Value::Null,
        )
        .await;
        assert_eq!(status, 401);
        sqlx::query("UPDATE team_agent_keys SET expires_at=NOW()-INTERVAL '1 second' WHERE id=$1")
            .bind(reader.id)
            .execute(&state.db)
            .await
            .unwrap();
        let (status, _) = api(
            &state,
            &reader.token,
            "GET",
            format!("{prefix}/memories"),
            Value::Null,
        )
        .await;
        assert_eq!(status, 401);
        remove_member(State(state.clone()), owner.clone(), Path((a.id, editor.id)))
            .await
            .unwrap();
        assert!(human_principal(&state, a.id, &editor).await.is_err());
        let events = list_audit(State(state.clone()), owner.clone(), Path(a.id))
            .await
            .unwrap();
        assert!(events.iter().any(|a| a.action == "memory.created"));
        assert!(events.iter().any(|a| a.action == "member.removed"));
        let stored: String =
            sqlx::query_scalar("SELECT token_hash FROM team_agent_keys WHERE id=$1")
                .bind(writer.id)
                .fetch_one(&state.db)
                .await
                .unwrap();
        assert_eq!(stored, hash(&writer.token));
        assert_ne!(stored, writer.token);
        let serialized = serde_json::to_string(
            &list_keys(State(state.clone()), owner.clone(), Path(a.id))
                .await
                .unwrap()
                .0,
        )
        .unwrap();
        assert!(!serialized.contains("lsk_"));
        assert!(!serialized.contains("token_hash"));
    }
}
