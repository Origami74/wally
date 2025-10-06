//! TollGate backend module
//!
//! This module handles all TollGate protocol operations, session management,
//! and background purchasing logic.

pub mod errors;
pub mod network;
pub mod protocol;
pub mod service;
pub mod session;
pub mod wallet;

pub use service::TollGateService;
// Re-export main types for external use when needed
#[allow(unused_imports)]
pub use errors::{TollGateError, TollGateResult};
#[allow(unused_imports)]
pub use protocol::TollGateProtocol;
#[allow(unused_imports)]
pub use session::{SessionManager, SessionParams};
