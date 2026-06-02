//! Diagnostic checks for OpenHuman configuration, workspace health, and daemon state.
//! Includes GATE闸机 (startup self-check) for OpenHuman-ZN — split into gate/ submodules.

mod core;
pub mod gate;
mod gate_extra_tests;
pub mod ops;
mod schemas;

pub use core::*;
pub use gate::*;
pub use ops as rpc;
pub use ops::*;
pub use schemas::{
    all_controller_schemas as all_doctor_controller_schemas,
    all_registered_controllers as all_doctor_registered_controllers,
};
