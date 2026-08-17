#!/usr/bin/env bash
# ============================================================
# VIRS 一键启动脚本
#
# 用法：
#   ./docker-start.sh          构建并启动所有服务（后台）
#   ./docker-start.sh -d       同上（显式指定后台模式）
#   ./docker-start.sh up       构建并启动（前台）
#   ./docker-start.sh stop     停止所有服务
#   ./docker-start.sh restart  重启所有服务
#   ./docker-start.sh logs     查看日志
#   ./docker-start.sh down     停止并删除容器、网络
#   ./docker-start.sh clean    停止、删除容器及数据卷（危险！）
# ============================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# ---- 颜色 ----
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

info()  { echo -e "${GREEN}[INFO]${NC}  $*"; }
warn()  { echo -e "${YELLOW}[WARN]${NC}  $*"; }
error() { echo -e "${RED}[ERROR]${NC} $*"; }

# ---- 检查 Docker ----
if ! command -v docker &>/dev/null; then
    error "未找到 docker，请先安装 Docker: https://docs.docker.com/get-docker/"
    exit 1
fi

if ! docker compose version &>/dev/null; then
    error "未找到 docker compose 插件，请升级 Docker 或安装 docker-compose"
    exit 1
fi

# ---- 生成随机密钥 ----
gen_hex() { openssl rand -hex 32; }
gen_pass() { openssl rand -base64 18 | tr -d '/+=' | head -c 24; }

# ---- 确保 .env 存在且包含必要密钥 ----
ensure_env() {
    local env_file="$SCRIPT_DIR/.env"
    local example_file="$SCRIPT_DIR/.env.example"

    if [[ ! -f "$env_file" ]]; then
        if [[ ! -f "$example_file" ]]; then
            error "未找到 .env 和 .env.example，无法继续"
            exit 1
        fi
        info ".env 不存在，从 .env.example 生成并填充随机密钥..."
        cp "$example_file" "$env_file"
        chmod 600 "$env_file"

        # 生成随机密钥并替换占位符
        local enc_key llm_key jwt_secret admin_pass
        enc_key=$(gen_hex)
        llm_key=$(gen_hex)
        jwt_secret=$(gen_hex)
        admin_pass=$(gen_pass)

        # macOS 和 Linux 兼容的 sed -i
        if [[ "$(uname)" == "Darwin" ]]; then
            sed -i '' \
                -e "s|^ENCRYPTION_KEY=.*|ENCRYPTION_KEY=${enc_key}|" \
                -e "s|^LLM_KEY=.*|LLM_KEY=${llm_key}|" \
                -e "s|^JWT_SECRET=.*|JWT_SECRET=${jwt_secret}|" \
                -e "s|^ADMIN_PASSWORD=.*|ADMIN_PASSWORD=${admin_pass}|" \
                "$env_file"
        else
            sed -i \
                -e "s|^ENCRYPTION_KEY=.*|ENCRYPTION_KEY=${enc_key}|" \
                -e "s|^LLM_KEY=.*|LLM_KEY=${llm_key}|" \
                -e "s|^JWT_SECRET=.*|JWT_SECRET=${jwt_secret}|" \
                -e "s|^ADMIN_PASSWORD=.*|ADMIN_PASSWORD=${admin_pass}|" \
                "$env_file"
        fi

        info ".env 已生成，随机密钥已填充"
        echo -e "  ${CYAN}ADMIN_USERNAME${NC}: admin"
        echo -e "  ${CYAN}ADMIN_PASSWORD${NC}: ${admin_pass}"
        echo ""
    else
        # .env 已存在，检查必填密钥是否仍为占位符
        local need_fix=()
        if grep -q '^ENCRYPTION_KEY=change-me' "$env_file"; then need_fix+=("ENCRYPTION_KEY"); fi
        if grep -q '^LLM_KEY=change-me' "$env_file"; then need_fix+=("LLM_KEY"); fi
        if grep -q '^JWT_SECRET=change-me' "$env_file"; then need_fix+=("JWT_SECRET"); fi
        if grep -q '^ADMIN_PASSWORD=change-this' "$env_file"; then need_fix+=("ADMIN_PASSWORD"); fi

        if [[ ${#need_fix[@]} -gt 0 ]]; then
            warn "检测到以下密钥仍为占位符: ${need_fix[*]}"
            warn "正在自动生成随机值替换..."
            local enc_key llm_key jwt_secret admin_pass
            enc_key=$(gen_hex)
            llm_key=$(gen_hex)
            jwt_secret=$(gen_hex)
            admin_pass=$(gen_pass)

            if [[ "$(uname)" == "Darwin" ]]; then
                sed -i '' \
                    -e "s|^ENCRYPTION_KEY=.*|ENCRYPTION_KEY=${enc_key}|" \
                    -e "s|^LLM_KEY=.*|LLM_KEY=${llm_key}|" \
                    -e "s|^JWT_SECRET=.*|JWT_SECRET=${jwt_secret}|" \
                    -e "s|^ADMIN_PASSWORD=.*|ADMIN_PASSWORD=${admin_pass}|" \
                    "$env_file"
            else
                sed -i \
                    -e "s|^ENCRYPTION_KEY=.*|ENCRYPTION_KEY=${enc_key}|" \
                    -e "s|^LLM_KEY=.*|LLM_KEY=${llm_key}|" \
                    -e "s|^JWT_SECRET=.*|JWT_SECRET=${jwt_secret}|" \
                    -e "s|^ADMIN_PASSWORD=.*|ADMIN_PASSWORD=${admin_pass}|" \
                    "$env_file"
            fi
            info "密钥已替换为随机值"
            echo -e "  ${CYAN}ADMIN_PASSWORD${NC}: ${admin_pass}"
            echo ""
        else
            info ".env 已存在且密钥已配置"
        fi
    fi
}

# ---- 启动 ----
start() {
    ensure_env
    info "构建并启动 Docker 服务..."
    docker compose up --build -d
    echo ""
    info "服务已启动"
    show_status
}

# ---- 前台启动 ----
start_foreground() {
    ensure_env
    info "构建并启动 Docker 服务（前台模式）..."
    docker compose up --build
}

# ---- 停止 ----
stop() {
    info "停止 Docker 服务..."
    docker compose stop
}

# ---- 重启 ----
restart() {
    info "重启 Docker 服务..."
    docker compose restart
}

# ---- 日志 ----
logs() {
    docker compose logs -f --tail=100
}

# ---- 销毁 ----
down() {
    warn "停止并删除容器、网络..."
    docker compose down
}

# ---- 清理（含数据卷） ----
clean() {
    warn "即将删除所有容器、网络及数据卷（数据库数据将丢失！）"
    read -rp "确认删除？输入 yes 继续: " confirm
    if [[ "$confirm" == "yes" ]]; then
        docker compose down -v
        info "已清理"
    else
        info "已取消"
    fi
}

# ---- 状态 ----
show_status() {
    echo ""
    echo -e "${CYAN}========== VIRS 服务状态 ==========${NC}"
    docker compose ps
    echo ""
    local backend_port
    backend_port=$(grep -E '^BACKEND_PORT=' .env 2>/dev/null | cut -d= -f2 || echo "8080")
    backend_port="${backend_port:-8080}"
    echo -e "${GREEN}访问地址:${NC}  http://localhost:${backend_port}"
    echo -e "${GREEN}管理账号:${NC}  admin（密码见 .env 中 ADMIN_PASSWORD）"
    echo -e "${CYAN}===================================${NC}"
    echo ""
    echo -e "常用命令:"
    echo -e "  ${CYAN}./docker-start.sh logs${NC}     查看日志"
    echo -e "  ${CYAN}./docker-start.sh stop${NC}     停止服务"
    echo -e "  ${CYAN}./docker-start.sh restart${NC}  重启服务"
    echo -e "  ${CYAN}./docker-start.sh down${NC}     销毁容器"
}

# ---- 帮助 ----
usage() {
    echo "VIRS Docker 一键启动脚本"
    echo ""
    echo "用法: ./docker-start.sh [命令]"
    echo ""
    echo "命令:"
    echo "  (无) / up -d   构建并后台启动"
    echo "  up              构建并前台启动"
    echo "  stop            停止服务"
    echo "  restart         重启服务"
    echo "  logs            查看日志"
    echo "  down            停止并删除容器"
    echo "  clean           删除容器及数据卷（危险！）"
    echo "  status          查看状态"
    echo "  help            显示帮助"
}

# ---- 主逻辑 ----
case "${1:-up-d}" in
    up-d|""|-d)
        start
        ;;
    up)
        start_foreground
        ;;
    stop)
        stop
        ;;
    restart)
        restart
        ;;
    logs)
        logs
        ;;
    down)
        down
        ;;
    clean)
        clean
        ;;
    status|ps)
        show_status
        ;;
    help|-h|--help)
        usage
        ;;
    *)
        error "未知命令: $1"
        usage
        exit 1
        ;;
esac
