# TollGate Rust Backend Design

## Overview

This document outlines the design for migrating TollGate app logic from TypeScript to Rust backend for proper background operation and session management.

## Current Issues

### Problems with Current Implementation
1. **UI Thread Logic**: All purchasing logic runs in TypeScript/Svelte UI layer
2. **Background Operation**: App stops working when backgrounded/locked
3. **Hardcoded Values**: Uses hardcoded private keys and "cashuAbcde" tokens
4. **No Renewal Logic**: No automatic session renewal when approaching expiration
5. **No Usage Tracking**: No monitoring of data/time consumption
6. **Complex UI**: Shows too much technical detail instead of simple toggle
7. **No Persistence**: Sessions don't survive app restarts

### What's Already Working
1. ✅ **Captive Portal Handling**: Android plugin properly handles captive portal intents
2. ✅ **Network Detection**: Automatic gateway IP and MAC address detection
3. ✅ **TollGate Discovery**: Checks port 2122 for pubkey and port 3334 for relay
4. ✅ **Auto-connection Flow**: Automatically detects and connects to tollgates

## Architecture Design

### Core Components

```
src-tauri/src/tollgate/
├── mod.rs              # Module exports
├── errors.rs           # Error types and handling
├── protocol.rs         # TollGate protocol implementation
├── session.rs          # Session management and state
├── service.rs          # Main TollGate service coordinator
├── wallet.rs           # Cashu wallet integration
└── network.rs          # Network detection and validation
```

### 1. TollGate Service (`service.rs`)

**Purpose**: Main coordinator that manages all TollGate operations

**Responsibilities**:
- Coordinate between network detection, protocol, sessions, and wallet
- Handle background monitoring and renewal
- Manage global enable/disable state
- Persist and restore sessions across app restarts

**Key Methods**:
```rust
impl TollGateService {
    pub async fn start() -> TollGateResult<Self>
    pub async fn enable_auto_tollgate(&self, enabled: bool) -> TollGateResult<()>
    pub async fn get_status(&self) -> ServiceStatus
    pub async fn handle_network_connected(&self, gateway_ip: String) -> TollGateResult<()>
    pub async fn handle_network_disconnected(&self) -> TollGateResult<()>
}
```

### 2. Session Management (`session.rs`)

**Purpose**: Track and manage individual TollGate sessions

**Session Lifecycle**:
1. **Discovery**: Network detected, TollGate validated
2. **Initial Purchase**: Create session with initial payment
3. **Active Monitoring**: Track usage and time remaining
4. **Renewal**: Automatic renewal at 80% threshold
5. **Expiration**: Handle session end and cleanup

**Key Structures**:
```rust
pub struct Session {
    pub id: String,
    pub tollgate_pubkey: String,
    pub gateway_ip: String,
    pub mac_address: String,
    pub status: SessionStatus,
    pub total_allotment: u64,
    pub current_usage: u64,
    pub session_end: chrono::DateTime<chrono::Utc>,
    pub renewal_threshold: f64, // 0.8 for 80%
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_renewal: Option<chrono::DateTime<chrono::Utc>>,
}

pub enum SessionStatus {
    Initializing,
    Active,
    Renewing,
    Expired,
    Error(String),
}
```

### 3. Protocol Implementation (`protocol.rs`)

**Purpose**: Handle TollGate protocol operations following reference implementation

**Protocol Flow**:
1. **Advertisement Discovery**: Parse kind 10021 events from port 2122
2. **Payment Creation**: Create kind 21000 events with real Cashu tokens
3. **Session Confirmation**: Listen for session response events
4. **Renewal Payments**: Handle automatic renewals

**Key Structures**:
```rust
pub struct TollGateAdvertisement {
    pub metric: String,           // "milliseconds" or "bytes"
    pub step_size: u64,
    pub pricing_options: Vec<PricingOption>,
    pub tips: Vec<String>,
}

pub struct PricingOption {
    pub asset_type: String,       // "cashu"
    pub price_per_step: u64,
    pub price_unit: String,       // "sat"
    pub mint_url: String,
    pub min_steps: u64,
}

pub struct PaymentEvent {
    pub kind: u16,                // 21000
    pub content: String,          // Real Cashu token
    pub tags: Vec<Vec<String>>,   // [["p", pubkey], ["mac", mac]]
}
```

### 4. Background Service

**Purpose**: Continuous monitoring and automatic operations

**Background Tasks**:
- **Session Monitoring**: Check all active sessions every 10 seconds
- **Usage Tracking**: Monitor time/data consumption
- **Renewal Triggers**: Automatic renewal at 80% threshold
- **Network Detection**: Detect new TollGate networks
- **Persistence**: Save/restore session state

**Implementation**:
```rust
impl TollGateService {
    async fn background_monitor(&self) {
        loop {
            self.check_all_sessions().await;
            self.detect_new_tollgates().await;
            self.persist_state().await;
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    }
    
    async fn check_session_renewal(&self, session: &Session) {
        let usage_percent = session.current_usage as f64 / session.total_allotment as f64;
        if usage_percent >= session.renewal_threshold {
            self.renew_session(session).await;
        }
    }
}
```

### 5. Wallet Integration (`wallet.rs`)

**Purpose**: Real Cashu wallet operations replacing hardcoded values

**Responsibilities**:
- Generate real Cashu tokens for payments
- Manage wallet balance and mints
- Handle payment creation and validation
- Integrate with CDK library

### 6. Network Detection (`network.rs`)

**Purpose**: Detect and validate TollGate networks

**Detection Flow**:
1. **Gateway Detection**: Get gateway IP from Android plugin
2. **TollGate Validation**: Check port 2122 for pubkey endpoint
3. **Advertisement Parsing**: Fetch and parse TollGate advertisement
4. **Compatibility Check**: Validate pricing options and mints

## User Experience Flow

### 1. App Launch
- Restore previous sessions from persistence
- Start background monitoring service
- Check current network for TollGate

### 2. Network Connection
- Android plugin detects network connection
- Rust service validates if it's a TollGate network
- If auto-tollgate enabled, automatically start purchasing

### 3. Session Management
- Create initial session with payment
- Monitor usage continuously in background
- Renew automatically at 80% threshold
- Handle network disconnections gracefully

### 4. UI Interaction
- Simple toggle button for enable/disable
- Real-time status display (connected/disconnected)
- Usage meter showing remaining time/data
- Minimal settings for wallet management

## API Design

### Tauri Commands

```rust
#[tauri::command]
async fn toggle_auto_tollgate(enabled: bool) -> Result<(), String>

#[tauri::command]
async fn get_tollgate_status() -> Result<ServiceStatus, String>

#[tauri::command]
async fn get_current_session() -> Result<Option<SessionInfo>, String>

#[tauri::command]
async fn force_session_renewal() -> Result<(), String>
```

### Events to Frontend

```rust
// Session status changes
emit("session-status-changed", SessionStatusEvent)

// Usage updates
emit("usage-updated", UsageUpdateEvent)

// Error notifications
emit("tollgate-error", ErrorEvent)
```

## Implementation Plan

### Phase 1: Core Infrastructure
1. ✅ Error handling system
2. ✅ Module structure
3. 🔄 Protocol implementation
4. 🔄 Session management
5. 🔄 Network detection

### Phase 2: Background Service
1. Main TollGate service coordinator
2. Background monitoring loop
3. Session persistence
4. Automatic renewal logic

### Phase 3: Wallet Integration
1. Real Cashu wallet implementation
2. Payment creation with real tokens
3. Balance management
4. Mint compatibility checking

### Phase 4: UI Simplification
1. Replace complex state display
2. Simple toggle interface
3. Clean status indicators
4. Minimal settings

### Phase 5: Testing & Optimization
1. Background operation testing
2. Session persistence validation
3. Network disconnection handling
4. Performance optimization

## Benefits of New Architecture

1. **True Background Operation**: Continues working when app is backgrounded
2. **Proper Protocol Implementation**: Follows reference implementation exactly
3. **Real Wallet Integration**: Uses actual Cashu tokens instead of hardcoded values
4. **Automatic Session Management**: Handles renewals and persistence automatically
5. **Simple User Experience**: Clean toggle interface instead of technical details
6. **Robust Error Handling**: Comprehensive error handling and recovery
7. **Performance**: Efficient Rust implementation with minimal resource usage