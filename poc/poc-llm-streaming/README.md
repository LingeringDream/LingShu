# Rust LLM Streaming PoC

验证 Rust reqwest 流式调用 Ollama 的可行性和性能。

## 运行方法

```bash
# 先启动 Ollama (需要本地安装或 Docker)
docker run -d --name ollama -p 11434:11434 ollama/ollama
docker exec ollama ollama pull qwen2.5:1.5b

# 运行 PoC
cargo run
```

## 验证测试

### Test 1: 基础流式传输
```bash
curl -X POST http://localhost:3000/chat \
  -H "Content-Type: application/json" \
  -d '{"message": "你好，请介绍一下自己"}'
```
- Pass: TTFT < 500ms
- Fail: 响应缓冲到完成才返回

### Test 2: 零拷贝转发
- 测量代理层开销: 目标 < 10ms/chunk
- 验证使用 `bytes_stream()` 而非收集完整 body

### Test 3: 并发流
```bash
# 10 个并发请求
for i in {1..10}; do
  curl -X POST http://localhost:3000/chat \
    -H "Content-Type: application/json" \
    -d "{\"message\": \"并发测试 $i\"}" &
done
wait
```
- Pass: 全部完成，无错误
- Fail: 内存增长超 50MB

### Test 4: 错误处理
- 中途停止 Ollama 容器
- 验证代理返回干净错误

## 结果记录

测试时间：2026-06-04

当前环境检查：

- 宿主机未发现 `ollama` CLI。
- `http://localhost:11434/api/tags` 无响应。
- 因缺少 Ollama 服务和模型，未执行 TTFT、并发和错误处理实测。

| 测试 | TTFT | 总时间 | Chunks | 内存 | 结果 |
|------|------|--------|--------|------|------|
| 基础流式 | N/A | N/A | N/A | N/A | BLOCKED: Ollama unavailable |
| 并发 x10 | N/A | N/A | N/A | N/A | BLOCKED: Ollama unavailable |
| 错误处理 | N/A | N/A | N/A | N/A | BLOCKED: Ollama unavailable |

## 结论

- [ ] PASS: TTFT < 500ms, 代理开销 < 10ms, 10 并发稳定
- [ ] FAIL: 需要调整配置或使用其他方案
- [x] BLOCKED: 当前环境缺少 Ollama 服务和模型，需先启动 Ollama 并拉取测试模型后再实测。
