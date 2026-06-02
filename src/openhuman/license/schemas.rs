// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OpenHuman-ZN Contributors
//! RPC controller schemas for the license domain.

use serde_json::{Map, Value};

use crate::core::all::{ControllerFuture, RegisteredController};
use crate::core::{ControllerSchema, FieldSchema, TypeSchema};

use super::ops;

pub fn license_schemas(function: &str) -> ControllerSchema {
    match function {
        "activate" => ControllerSchema {
            namespace: "license",
            function: "activate",
            description: "激活许可证密钥（在线验证或离线降级）",
            inputs: vec![
                FieldSchema {
                    name: "license_key",
                    ty: TypeSchema::String,
                    comment: "许可证密钥 (XXXX-XXXX-XXXX-XXXX)",
                    required: true,
                },
                FieldSchema {
                    name: "user_email",
                    ty: TypeSchema::String,
                    comment: "用户邮箱",
                    required: true,
                },
            ],
            outputs: vec![FieldSchema {
                name: "result",
                ty: TypeSchema::Json,
                comment: "激活结果 (LicenseActivationResponse)",
                required: true,
            }],
        },
        "status" => ControllerSchema {
            namespace: "license",
            function: "status",
            description: "获取当前许可证状态和用量",
            inputs: vec![],
            outputs: vec![FieldSchema {
                name: "result",
                ty: TypeSchema::Json,
                comment: "许可证状态 (LicenseInfo)",
                required: true,
            }],
        },
        "clear" => ControllerSchema {
            namespace: "license",
            function: "clear",
            description: "清除许可证状态并重置所有用量",
            inputs: vec![],
            outputs: vec![FieldSchema {
                name: "ok",
                ty: TypeSchema::Bool,
                comment: "操作是否成功",
                required: true,
            }],
        },
        _ => ControllerSchema {
            namespace: "license",
            function: "",
            description: "",
            inputs: vec![],
            outputs: vec![],
        },
    }
}

pub fn all_license_controller_schemas() -> Vec<ControllerSchema> {
    vec![
        license_schemas("activate"),
        license_schemas("status"),
        license_schemas("clear"),
    ]
}

pub fn all_license_registered_controllers() -> Vec<RegisteredController> {
    vec![
        RegisteredController {
            schema: license_schemas("activate"),
            handler: handle_license_activate,
        },
        RegisteredController {
            schema: license_schemas("status"),
            handler: handle_license_status,
        },
        RegisteredController {
            schema: license_schemas("clear"),
            handler: handle_license_clear,
        },
    ]
}

fn handle_license_activate(mut params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        let license_key = params
            .remove("license_key")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default();
        let user_email = params
            .remove("user_email")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default();
        let api_url = "https://api.openhuman-zn.cn".to_string();

        match ops::activate_license(&license_key, &user_email, &api_url).await {
            Ok(resp) => Ok(serde_json::to_value(resp).unwrap_or_default()),
            Err(e) => Err(format!("{e}")),
        }
    })
}

fn handle_license_status(_params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        let info = ops::get_license_info();
        Ok(serde_json::to_value(info).unwrap_or_default())
    })
}

fn handle_license_clear(_params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        ops::clear_license();
        Ok(serde_json::json!({"ok": true}))
    })
}
