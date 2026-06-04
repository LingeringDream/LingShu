# Qdrant Vector Search PoC

验证 Qdrant HNSW 向量检索在 100K 向量规模下的性能。

## 运行方法

```bash
docker compose up -d
cargo run
```

## 验证指标

| 指标 | 目标 | 实际 |
|------|------|------|
| P95 搜索延迟 | < 5ms | - |
| 带过滤 P95 | < 10ms | - |
| 内存占用 | < 512MB | - |
| 插入吞吐量 | - | - |

## 结果记录

- 测试时间: 2026-06-04
- 运行方式: `docker run --rm --network docker_default -e QDRANT_URL=http://qdrant:6333 -v /Users/ymqz/projects/PA:/app -w /app/poc/poc-qdrant-search docker-backend cargo run --release`
- Qdrant: dev compose `qdrant/qdrant:latest`
- 向量规模: 100,000
- 向量维度: 768
- 查询次数: 1,000
- 插入时间: 11.03s
- 插入吞吐量: 9,067 vectors/sec
- P50/P95/P99: 3.510625ms / 14.486042ms / 24.594458ms
- 过滤搜索 P50/P95/P99: 6.060583ms / 29.817167ms / 48.736292ms

## 结论

- [ ] PASS: P95 < 5ms, 内存 < 512MB
- [ ] PARTIAL: P95 < 10ms (可接受但需优化)
- [x] FAIL: 当前 100K 随机向量测试未达到 P95 < 5ms / filtered P95 < 10ms，需优化 collection/index 参数、过滤字段索引、payload 策略，或重新评估目标阈值。
