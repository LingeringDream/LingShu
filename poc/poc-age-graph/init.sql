-- Install AGE extension
CREATE EXTENSION IF NOT EXISTS age;
LOAD 'age';
SET search_path = ag_catalog, "$user", public;

-- Create graph
SELECT create_graph('project_graph');

-- Create vertex labels
SELECT create_vlabel('project_graph', 'Person');
SELECT create_vlabel('project_graph', 'Task');
SELECT create_vlabel('project_graph', 'Document');

-- Create edge labels
SELECT create_elabel('project_graph', 'RESPONSIBLE_FOR');
SELECT create_elabel('project_graph', 'DEPENDS_ON');
SELECT create_elabel('project_graph', 'DOCUMENTED_IN');

-- Insert test data: 100 persons
SELECT * FROM cypher('project_graph', $$
    UNWIND range(1, 100) AS i
    CREATE (:Person {id: i, name: 'Person_' + toString(i)})
$$) AS (v agtype);

-- Insert test data: 500 tasks
SELECT * FROM cypher('project_graph', $$
    UNWIND range(1, 500) AS i
    CREATE (:Task {id: i, title: 'Task_' + toString(i), status: CASE WHEN i % 5 = 0 THEN 'done' WHEN i % 3 = 0 THEN 'in_progress' ELSE 'todo' END})
$$) AS (v agtype);

-- Insert test data: 200 documents
SELECT * FROM cypher('project_graph', $$
    UNWIND range(1, 200) AS i
    CREATE (:Document {id: i, name: 'Doc_' + toString(i)})
$$) AS (v agtype);

-- Create edges: Person -[:RESPONSIBLE_FOR]-> Task (each person responsible for 3-8 tasks)
SELECT * FROM cypher('project_graph', $$
    MATCH (p:Person), (t:Task)
    WHERE t.id % 100 = p.id % 100 AND t.id <= p.id * 5
    CREATE (p)-[:RESPONSIBLE_FOR]->(t)
$$) AS (v agtype);

-- Create edges: Task -[:DEPENDS_ON]-> Task (each task depends on 0-3 previous tasks)
SELECT * FROM cypher('project_graph', $$
    MATCH (t1:Task), (t2:Task)
    WHERE t1.id > t2.id AND t1.id - t2.id <= 3 AND t1.id % 4 != 0
    CREATE (t1)-[:DEPENDS_ON]->(t2)
$$) AS (v agtype);

-- Create edges: Task -[:DOCUMENTED_IN]-> Document
SELECT * FROM cypher('project_graph', $$
    MATCH (t:Task), (d:Document)
    WHERE d.id = t.id % 200 + 1
    CREATE (t)-[:DOCUMENTED_IN]->(d)
$$) AS (v agtype);

-- Print stats
SELECT 'Persons' as label, count(*) FROM cypher('project_graph', $$ MATCH (p:Person) RETURN count(p) $$) AS (cnt agtype)
UNION ALL
SELECT 'Tasks', count(*) FROM cypher('project_graph', $$ MATCH (t:Task) RETURN count(t) $$) AS (cnt agtype)
UNION ALL
SELECT 'Documents', count(*) FROM cypher('project_graph', $$ MATCH (d:Document) RETURN count(d) $$) AS (cnt agtype);
