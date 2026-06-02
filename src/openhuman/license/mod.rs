// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OpenHuman-ZN Contributors
//
//! License domain — commercial activation, quota enforcement, device binding.
//!
//! IMPORTANT: This module is part of the GPL-3.0 codebase. Commercial use is via
//! SaaS-hosted license verification service, NOT by closing this source.
//!
//! RPC controllers: `openhuman.license_activate`, `openhuman.license_status`,
//! `openhuman.license_clear`.

mod ops;
mod ops_tests;
mod schemas;
pub mod types;

pub use ops::*;
pub use schemas::{
    all_license_controller_schemas, all_license_registered_controllers, license_schemas,
};
pub use types::*;
