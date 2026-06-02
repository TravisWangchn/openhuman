//! Billing and payment RPC adapters that thin-wrap the hosted API.
//!
//! OpenHuman-ZN adds WeChat Pay / Alipay via `china_payments`.

pub mod china_payments;
mod china_payments_tests;
mod ops;
mod schemas;

pub use china_payments::*;
pub use ops::*;
pub use schemas::{
    all_billing_controller_schemas, all_billing_registered_controllers, billing_schemas,
};
