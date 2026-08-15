/*
 * Copyright(c) 2024-2026 China Telecom Cloud Technologies Co., Ltd. All rights
 * reserved. track-system is licensed under Mulan PSL v2. You can use this software
 * according to the terms and conditions of the Mulan PSL V2. You may obtain a
 * copy of Mulan PSL v2 at: http://license.coscl.org.cn/MulanPSL2.
 * THIS SOFTWARE IS PROVIDED ON AN "AS IS" BASIS, WITHOUT WARRANTIES OF ANY
 * KIND, EITHER EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO NON-INFRINGEMENT,
 * MERCHANTABILITY OR FIT FOR A PARTICULAR PURPOSE.  See the Mulan PSL v2 for
 * more details.
 */

use std::path::Path;

const TRACK_SYSTEM_ENV_FILES: [&str; 2] =
    ["track-system.env", "/etc/track-system/track-system.env"];

pub fn load_track_system_env() {
    for env_file in TRACK_SYSTEM_ENV_FILES {
        if Path::new(env_file).exists() {
            let _ = dotenvy::from_filename(env_file);
        }
    }
}
