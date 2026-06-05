use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Graph node types in the knowledge graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeType {
    Person,
    Task,
    Document,
    Decision,
    Risk,
    Milestone,
}

/// Graph edge types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EdgeType {
    ResponsibleFor,
    DependsOn,
    BelongsTo,
    DocumentedBy,
    Impacts,
    RelatedTo,
}

/// Execute a dependency chain query using Apache AGE
pub async fn query_dependency_chain(
    pool: &sqlx::PgPool,
    task_id: Uuid,
    max_depth: i32,
) -> anyhow::Result<Vec<GraphNode>> {
    let query = format!(
        r#"
        SELECT * FROM cypher('project_knowledge', $$
            MATCH (t:Task {{id: '{task_id}'}})-[:DEPENDS_ON*1..{max_depth}]->(dep:Task)
            RETURN dep.id, dep.title, dep.status
        $$) AS (task_id agtype, title agtype, status agtype)
        "#
    );

    let rows = sqlx::query_as::<_, GraphQueryRow>(&query)
        .fetch_all(pool)
        .await?;

    Ok(rows.into_iter().map(|r| r.into()).collect())
}

/// Execute an impact analysis query
pub async fn query_impact_analysis(
    pool: &sqlx::PgPool,
    task_id: Uuid,
    max_depth: i32,
) -> anyhow::Result<Vec<GraphNode>> {
    let query = format!(
        r#"
        SELECT * FROM cypher('project_knowledge', $$
            MATCH (t:Task {{id: '{task_id}'}})<-[:DEPENDS_ON*1..{max_depth}]-(affected:Task)
            WHERE affected.status <> 'done'
            RETURN affected.id, affected.title, affected.status
        $$) AS (task_id agtype, title agtype, status agtype)
        "#
    );

    let rows = sqlx::query_as::<_, GraphQueryRow>(&query)
        .fetch_all(pool)
        .await?;

    Ok(rows.into_iter().map(|r| r.into()).collect())
}

#[derive(Debug, sqlx::FromRow)]
struct GraphQueryRow {
    task_id: String,
    title: String,
    status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: Uuid,
    pub title: String,
    pub status: String,
}

impl From<GraphQueryRow> for GraphNode {
    fn from(row: GraphQueryRow) -> Self {
        Self {
            id: Uuid::parse_str(&row.task_id).unwrap_or_default(),
            title: row.title,
            status: row.status,
        }
    }
}
