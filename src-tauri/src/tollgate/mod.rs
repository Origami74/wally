//! TollGate backend module
//! 
//! This module handles all TollGate protocol operations, session management,
//! and background purchasing logic.

pub mod protocol;
pub mod session;
pub mod service;
pub mod wallet;
pub mod network;
pub mod errors;

pub use service::TollGateService;
// Re-export main types for external use when needed
#[allow(unused_imports)]
pub use session::{SessionManager};
#[allow(unused_imports)]
pub use protocol::{TollGateProtocol};
#[allow(unused_imports)]
pub use errors::{TollGateError, TollGateResult};