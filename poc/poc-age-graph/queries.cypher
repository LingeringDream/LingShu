-- Query 1: Dependency chain traversal (2-hop minimum)
-- Pass criteria: < 50ms for task with 3 levels of dependencies
EXPLAIN ANALYZE
SELECT * FROM cypher('project_graph', $$
    MATCH (t:Task {id: 100})-[:DEPENDS_ON*1..3]->(dep:Task)
    RETURN dep.id, dep.title, dep.status
$$) AS (task_id agtype, title agtype, status agtype);

-- Query 2: Person-Task-Document path query
-- Pass criteria: Works with AGE Cypher syntax
EXPLAIN ANALYZE
SELECT * FROM cypher('project_graph', $$
    MATCH (p:Person {id: 1})-[:RESPONSIBLE_FOR]->(t:Task)-[:DOCUMENTED_IN]->(d:Document)
    RETURN p.name, t.title, d.name
$$) AS (name agtype, title agtype, doc_name agtype);

-- Query 3: Impact analysis (multi-hop with filtering)
-- Pass criteria: < 200ms for graph with 500 tasks and 1000 edges
EXPLAIN ANALYZE
SELECT * FROM cypher('project_graph', $$
    MATCH (t:Task {id: 200})<-[:DEPENDS_ON*1..5]-(affected:Task)
    WHERE affected.status <> 'done'
    RETURN affected.id, affected.title, affected.status
$$) AS (task_id agtype, title agtype, status agtype);
