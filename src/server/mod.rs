/*
 * Copyright(c) 2024-2026 China Telecom Cloud Technologies Co., Ltd. All rights
 * reserved. ctscat is licensed under Mulan PSL v2. You can use this software
 * according to the terms and conditions of the Mulan PSL V2. You may obtain a
 * copy of Mulan PSL v2 at: http://license.coscl.org.cn/MulanPSL2.
 * THIS SOFTWARE IS PROVIDED ON AN "AS IS" BASIS, WITHOUT WARRANTIES OF ANY
 * KIND, EITHER EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO NON-INFRINGEMENT,
 * MERCHANTABILITY OR FIT FOR A PARTICULAR PURPOSE.  See the Mulan PSL v2 for
 * more details.
 */

pub mod api;
pub mod dto;
pub mod error;
pub mod handlers;
pub mod middleware;
pub mod routes;
pub mod state;

use axum::Router;
use sea_orm::DatabaseConnection;
use std::{env, net::SocketAddr, sync::Arc};

use crate::collectors::{gitea::GiteaClient, gitee::GiteeClient};

use self::{
    routes::{
