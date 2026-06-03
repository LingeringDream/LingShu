# Apache AGE PoC

验证 Apache AGE 在 PostgreSQL 16 上的图查询能力和性能。

## 运行方法

```bash
docker compose up -d
docker compose exec postgres psql -U poc -d poc_age -f /docker-entrypoint-initdb.d/queries.cypher
```

## 验证查询

### Query 1: 依赖链遍历 (2-3 跳)
```sql
SELECT * FROM cypher('project_graph', $$
    MATCH (t:Task {id: 100})-[:DEPENDS_ON*1..3]->(dep:Task)
    RETURN dep.id, dep.title, dep.status
$$) AS (task_id agtype, title agtype, status agtype);
```
- Pass: < 50ms
- Fail: > 200ms 或语法不支持

### Query 2: 人-任务-文档路径查询
```sql
SELECT * FROM cypher('project_graph', $$
    MATCH (p:Person {id: 1})-[:RESPONSIBLE_FOR]->(t:Task)-[:DOCUMENTED_IN]->(d:Document)
    RETURN p.name, t.title, d.name
$$) AS (name agtype, title agtype, doc_name agtype);
```
- Pass: 正确返回结果
- Fail: Cypher 语法不支持

### Query 3: 影响分析 (多跳 + 过滤)
```sql
SELECT * FROM cypher('project_graph', $$
    MATCH (t:Task {id: 200})<-[:DEPENDS_ON*1..5]-(affected:Task)
    WHERE affected.status <> 'done'
    RETURN affected.id, affected.title, affected.status
$$) AS (task_id agtype, title agtype, status agtype);
```
- Pass: < 200ms
- Fail: > 1s 或结果不正确

## 结果记录

| 查询 | 执行时间 | 结果 | 备注 |
|------|---------|------|------|
| Q1 依赖链 | - | - | |
| Q2 路径查询 | - | - | |
| Q3 影响分析 | - | - | |

## 结论

- [ ] PASS: 全部通过
- [ ] PARTIAL: 可用但需优化
- [ ] FAIL: 需要回退到 Neo4j
