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
