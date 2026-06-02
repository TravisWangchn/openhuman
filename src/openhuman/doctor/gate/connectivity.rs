// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OpenHuman-ZN Contributors

//! GATE-3: Model endpoint connectivity (deferred).
//! GATE-4: Payment webhook reachability (deferred).

use super::{make_result, GateCheckResult, GateCode, GateSeverity};

/// GATE-3: Validates that each configured CN model provider endpoint can be
/// reached.
///
/// **Network calls are deferred**: this gate always passes with a note that
/// connectivity will be verified on the first actual inference call, so that
/// a transient network issue does not block startup.
pub fn check_model_connectivity() -> GateCheckResult {
    make_result(
        "GATE-3 模型连通性",
        true,
        "连接性将在首次推理调用时验证",
        GateSeverity::Warning,
        None,
        Some(GateCode::Gate3ModelConnectivity),
    )
}

/// GATE-4: Verifies the payment callback URL endpoint is reachable.
///
/// **Network calls are deferred**: this gate always passes with a note that
/// webhook reachability will be verified on the first payment event, so
/// that the system does not refuse to start just because the billing
/// backend is momentarily unreachable.
pub fn check_payment_webhook() -> GateCheckResult {
    make_result(
        "GATE-4 支付回调",
        true,
        "Webhook 将在首次支付事件时验证",
        GateSeverity::Warning,
        None,
        Some(GateCode::Gate4PaymentWebhook),
    )
}
