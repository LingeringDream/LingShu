#!/usr/bin/env bash
# 一条命令本地调试（浏览器模式，无需编译 Tauri）。
# 拉起 docker 基础设施 → 跑数据库迁移 → 并发启动后端(8080)+前端(5173)。
# 可从任意目录运行——自动定位仓库根。前端 isTauri() 会在浏览器里把
# EventKit / 宠物窗口等桌面专属能力优雅降级，其余功能照常调试。
set -euo pipefail

cd "$(dirname "$0")/.."
ROOT="$PWD"

COMPOSE="docker compose -f docker/docker-compose.dev.yml"

usage() {
    cat <<'EOF'
用法: ./scripts/dev.sh [选项]
  (无)        浏览器模式：基础设施 + 迁移 + 后端(8080) + 前端(5173)
  --watch     后端热重载（需 cargo-watch，未装则回退普通 cargo run）
  --tauri     Tauri 桌面模式（编译桌面壳，已含 vite）
  --down      停止 docker 基础设施
  --help      显示本帮助

浏览器打开 http://localhost:5173 即可调试。
EOF
}

# ── 解析参数 ───────────────────────────────────────────
MODE="browser" # browser | tauri
WATCH=0
for arg in "$@"; do
    case "$arg" in
        --down) echo "→ 停止基础设施…"; $COMPOSE down; echo "✓ 已停止"; exit 0 ;;
        --tauri) MODE="tauri" ;;
        --watch) WATCH=1 ;;
        --help | -h) usage; exit 0 ;;
        *) echo "✗ 未知参数: ${arg}（见 --help）" >&2; exit 1 ;;
    esac
done

# ── 预检 ───────────────────────────────────────────────
command -v docker >/dev/null 2>&1 || { echo "✗ 未找到 docker" >&2; exit 1; }
docker info >/dev/null 2>&1 || { echo "✗ Docker 守护进程未运行，请先启动 Docker Desktop" >&2; exit 1; }

[ -f "$ROOT/config.toml" ] || { echo "✗ 缺少 config.toml，请执行: cp config.example.toml config.toml" >&2; exit 1; }
[ -f "$ROOT/.env" ] || { echo "✗ 缺少 .env，请执行: cp .env.example .env  （并把 DATABASE_URL 等改为本地 localhost 地址）" >&2; exit 1; }

command -v sqlx >/dev/null 2>&1 || { echo "✗ 未找到 sqlx-cli，请执行: cargo install sqlx-cli --no-default-features --features rustls,postgres" >&2; exit 1; }

# 原生模式连的是 docker 发布到 127.0.0.1 的端口；若 .env 指向 docker 内部主机名会连不上。
if grep -qE '^DATABASE_URL=.*@postgres:' "$ROOT/.env" 2>/dev/null; then
    echo "⚠ .env 的 DATABASE_URL 指向 docker 内部主机名 postgres；原生模式应使用 localhost。" >&2
fi

if [ ! -d "$ROOT/frontend/node_modules" ]; then
    echo "→ 安装前端依赖 (npm ci)…"
    (cd "$ROOT/frontend" && npm ci)
fi

# 端口预检：docker 要把 5432/6379/6333 发布到 127.0.0.1；若已被本机非容器进程
# （如开机自启的原生 Postgres）占用，docker 会起不来——提前给出清晰提示。
check_port() {
    local port="$1" svc="$2"
    if lsof -nP -iTCP:"$port" -sTCP:LISTEN >/dev/null 2>&1 &&
        ! docker ps --filter "publish=$port" --format '{{.Names}}' | grep -q .; then
        echo "✗ 端口 ${port}（${svc}）已被本机非容器进程占用，docker 容器无法绑定。" >&2
        echo "  查看占用: lsof -nP -iTCP:${port} -sTCP:LISTEN" >&2
        return 1
    fi
}
conflict=0
check_port 5432 postgres || conflict=1
check_port 6379 redis || conflict=1
check_port 6333 qdrant || conflict=1
if [ "$conflict" -eq 1 ]; then
    echo "  若是原生 Postgres：brew services stop postgresql  或退出 Postgres.app，然后重跑。" >&2
    exit 1
fi

# 后端端口预检：原生后端要绑 8080。最常见的坑是已安装的「灵枢」App 仍在运行、
# 占着 8080——此时本脚本的 cargo 后端会绑定失败并退出，而 vite 前端仍会启动并把
# /api 请求代理到那个旧后端上，接口表现为 404（且新旧版本路由不一致会更难排查）。
if lsof -nP -iTCP:8080 -sTCP:LISTEN >/dev/null 2>&1; then
    echo "✗ 端口 8080 已被占用：本地后端无法启动。" >&2
    echo "  多半是已安装的「灵枢 LingShu」App 仍在运行——请先退出它再重跑本脚本。" >&2
    echo "  查看占用: lsof -nP -iTCP:8080 -sTCP:LISTEN" >&2
    exit 1
fi

# ── 基础设施 (postgres / redis / qdrant) ───────────────
echo "→ 启动基础设施 (postgres / redis / qdrant)…"
$COMPOSE up -d --wait

# ── 数据库迁移 ─────────────────────────────────────────
echo "→ 运行数据库迁移…"
sqlx migrate run --source crates/lingshu-server/migrations

# ── 后端命令 ───────────────────────────────────────────
if [ "$WATCH" -eq 1 ] && command -v cargo-watch >/dev/null 2>&1; then
    SERVER=(cargo watch -x "run -p lingshu-server")
elif [ "$WATCH" -eq 1 ]; then
    echo "⚠ 未安装 cargo-watch，回退普通 cargo run（cargo install cargo-watch 可启用热重载）" >&2
    SERVER=(cargo run -p lingshu-server)
else
    SERVER=(cargo run -p lingshu-server)
fi

# 为每一路日志加前缀；用纯 bash read 循环，逐行立即输出（macOS sed 无 -u）。
prefix() {
    local tag="$1"
    while IFS= read -r line || [ -n "$line" ]; do
        printf '%s %s\n' "$tag" "$line"
    done
}

# Ctrl-C / 退出时结束整个进程组（含 cargo/vite 的孙进程），避免孤儿进程。
trap 'trap - INT TERM EXIT; echo; echo "→ 关闭前后端…"; kill 0' INT TERM EXIT

(cd "$ROOT" && exec "${SERVER[@]}" 2>&1 | prefix "[server]") &

if [ "$MODE" = "tauri" ]; then
    echo ""
    echo "✓ 就绪：编译并启动 Tauri 桌面壳（已含 vite 前端）。后端 http://127.0.0.1:8080"
    echo "  按 Ctrl-C 结束全部进程。"
    (cd "$ROOT/frontend" && exec ./node_modules/.bin/tauri dev 2>&1 | prefix "[tauri]") &
else
    (cd "$ROOT/frontend" && exec npm run dev 2>&1 | prefix "[web]") &
    echo ""
    echo "✓ 就绪：浏览器打开 http://localhost:5173  （后端 http://127.0.0.1:8080）"
    echo "  按 Ctrl-C 结束全部进程。"
fi

wait
