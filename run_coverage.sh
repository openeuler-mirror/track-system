#!/bin/bash
#
# Copyright(c) 2024-2026 China Telecom Cloud Technologies Co., Ltd. All rights
# reserved. ctscat is licensed under Mulan PSL v2. You can use this software
# according to the terms and conditions of the Mulan PSL V2. You may obtain a
# copy of Mulan PSL v2 at: http://license.coscl.org.cn/MulanPSL2.
# THIS SOFTWARE IS PROVIDED ON AN "AS IS" BASIS, WITHOUT WARRANTIES OF ANY
# KIND, EITHER EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO NON-INFRINGEMENT,
# MERCHANTABILITY OR FIT FOR A PARTICULAR PURPOSE.  See the Mulan PSL v2 for
# more details.
#

# TRACK-SYSTEM 测试覆盖率自动化脚本
# 用途: 运行 Tarpaulin 生成覆盖率报告,解析结果并记录到 checklist

set -e  # 遇到错误立即退出

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 默认参数
COVERAGE_THRESHOLD=${1:-50}  # 默认阈值 50%
TIMEOUT=240                   # 默认超时 240 秒
OUTPUT_FORMAT="Html"          # 默认输出格式
REPORT_FILE="tarpaulin-report.html"
CHECKLIST_FILE="docs/coverage/checklist.md"

# 打印带颜色的信息
print_info() {
    echo -e "${BLUE}ℹ️  $1${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

print_header() {
    echo ""
    echo -e "${BLUE}================================================${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}================================================${NC}"
    echo ""
}

# 帮助信息
show_help() {
    echo "用法: $0 [覆盖率阈值]"
    echo ""
    echo "参数:"
    echo "  覆盖率阈值    期望的最低覆盖率百分比 (默认: 50)"
    echo ""
    echo "示例:"
    echo "  $0          # 使用默认阈值 50%"
    echo "  $0 80       # 要求覆盖率 >= 80%"
    echo "  $0 90       # 要求覆盖率 >= 90%"
    echo ""
    echo "环境变量:"
    echo "  TIMEOUT               Tarpaulin 超时时间 (秒, 默认: 240)"
    echo "  OUTPUT_FORMAT         输出格式 (默认: Html)"
    echo "  SKIP_THRESHOLD_CHECK  跳过阈值检查 (设置为 1)"
    exit 0
}

# 检查参数
if [ "$1" = "-h" ] || [ "$1" = "--help" ]; then
    show_help
fi

# 验证阈值参数
if ! [[ "$COVERAGE_THRESHOLD" =~ ^[0-9]+$ ]] || [ "$COVERAGE_THRESHOLD" -lt 0 ] || [ "$COVERAGE_THRESHOLD" -gt 100 ]; then
    print_error "无效的覆盖率阈值: $COVERAGE_THRESHOLD"
    echo "阈值必须是 0-100 之间的整数"
    exit 1
fi

print_header "TRACK-SYSTEM 测试覆盖率检查"

print_info "覆盖率阈值: ${COVERAGE_THRESHOLD}%"
print_info "超时时间: ${TIMEOUT} 秒"
print_info "输出格式: ${OUTPUT_FORMAT}"
echo ""

# 1. 检查 Tarpaulin 是否已安装
print_info "检查 cargo-tarpaulin 是否已安装..."
if ! command -v cargo-tarpaulin &> /dev/null; then
    print_warning "cargo-tarpaulin 未安装,正在安装..."
    cargo install cargo-tarpaulin || {
        print_error "cargo-tarpaulin 安装失败"
        echo ""
        echo "请手动安装: cargo install cargo-tarpaulin"
        exit 1
    }
    print_success "cargo-tarpaulin 安装成功"
else
    TARPAULIN_VERSION=$(cargo tarpaulin --version | head -1)
    print_success "已安装: $TARPAULIN_VERSION"
fi
echo ""

# 2. 清理旧的报告文件
print_info "清理旧的报告文件..."
if [ -f "$REPORT_FILE" ]; then
    rm -f "$REPORT_FILE"
    print_success "已删除旧报告: $REPORT_FILE"
fi
echo ""

# 3. 运行 Tarpaulin
print_header "运行 Tarpaulin 测试覆盖率分析"
print_info "这可能需要几分钟时间,请耐心等待..."
echo ""

START_TIME=$(date +%s)

# 运行 Tarpaulin 并捕获输出
TARPAULIN_OUTPUT=$(cargo tarpaulin \
    --workspace \
    --timeout $TIMEOUT \
    --out $OUTPUT_FORMAT \
    --engine llvm \
    --verbose \
    --exclude-files "migration/*"
    2>&1) || {
    TARPAULIN_EXIT_CODE=$?
    print_error "Tarpaulin 运行失败 (退出码: $TARPAULIN_EXIT_CODE)"
    echo ""
    echo "错误输出:"
