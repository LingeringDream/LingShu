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

- 插入时间: -
- P50/P95/P99: -
- 过滤搜索 P50/P95/P99: -

## 结论

- [ ] PASS: P95 < 5ms, 内存 < 512MB
- [ ] PARTIAL: P95 < 10ms (可接受但需优化)
- [ ] FAIL: 需要回退到 pgvector 或 Milvus
