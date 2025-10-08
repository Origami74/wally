# Cashu Wallet Connect (CWC) Specification

**Version:** 0.1.0  
**Status:** Draft  
**Authors:** Wally Contributors  
**Based on:** [NIP-47 (Nostr Wallet Connect)](https://github.com/nostr-protocol/nips/blob/master/47.md)

## Abstract

Cashu Wallet Connect (CWC) extends NIP-47 (Nostr Wallet Connect) to support Cashu ecash wallets. It maintains NWC compatibility while adding native Cashu token operations, multi-mint support, and NUT-18 payment requests.

## How CWC Differs from NWC

CWC extends the standard [NWC protocol](https://docs.nwc.dev/) with Cashu-specific functionality:

| Aspect | Standard NWC | CWC |
|--------|-------------|-----|
| **Backend** | Lightning node | Cashu mints |
| **Primary Asset** | Lightning payments | Cashu ecash tokens |
| **Payment Types** | BOLT11 invoices only | BOLT11 + NUT-18 payment requests + raw tokens |
| **Balance Source** | Single Lightning node | Multiple mints (aggregated) |
| **Methods** | 8 standard methods | 3 standard + 2 Cashu-specific |
| **Custody Model** | Non-custodial (if self-hosted node) | Custodial (mint-issued ecash) |
| **Network** | Lightning Network | Cashu protocol |

### Additional Methods

CWC adds two Cashu-specific methods:

- **`receive_cashu`**: Accept and validate Cashu tokens
- **`pay_cashu_request`**: Pay NUT-18 payment requests (with or without transport)

### Extended Responses

The `get_balance` response includes multi-mint breakdown:

```json
{
  "balance": 50000,
  "max_sendable": 30000,
  "mints": [
    { "mint_url": "...", "balance": 30000, "unit": "msat" }
  ]
}
```

### Not Implemented

The following standard NWC methods are not implemented:

- ❌ `get_info`
- ❌ `multi_pay_invoice`
- ❌ `pay_keysend`
- ❌ `lookup_invoice`
- ❌ `list_transactions`

## Protocol Overview

CWC uses [NIP-47](https://github.com/nostr-protocol/nips/blob/master/47.md) event kinds and encryption:

- **Requests**: Kind `23194` (WalletConnectRequest), NIP-04 encrypted
- **Responses**: Kind `23195` (WalletConnectResponse), NIP-04 encrypted
- **Info**: Kind `13194` (WalletConnectInfo)
- **Transport**: Nostr relays

## Connection Types

### Standard NWC Connection

**URI Format:**
```
nostr+walletconnect://<hex-pubkey>?relay=<relay-url>&secret=<hex-secret>
```

Wallet generates unique keypair per connection. App signs requests with the secret.

### Nostr Wallet Auth (NWA)

App initiates with its own pubkey → Wallet approves → Broadcasts kind `33194` event with connection details. App keeps its own keys.

## Methods

### Standard NIP-47 Methods

#### `get_balance`

Returns total balance across all mints with per-mint breakdown.

**Response:**
```json
{
  "balance": 50000,
  "max_sendable": 30000,
  "mints": [
    { "mint_url": "https://mint.example.com", "balance": 30000, "unit": "msat" }
  ]
}
```

#### `make_invoice`

Creates BOLT11 invoice via default mint's Lightning gateway.

**Request:**
```json
{
  "method": "make_invoice",
  "params": { "amount": 10000, "description": "..." }
}
```

**Response:**
```json
{
  "invoice": "lnbc100n1...",
  "payment_hash": "abc123..."
}
```

#### `pay_invoice`

Pays BOLT11 invoice using Cashu tokens. Checks budget before payment.

**Request:**
```json
{
  "method": "pay_invoice",
  "params": { "invoice": "lnbc100n1..." }
}
```

**Response:**
```json
{
  "preimage": "def456...",
  "fees_paid": 100
}
```

### Cashu-Specific Methods

#### `receive_cashu`

Accepts and validates a Cashu token. Automatically adds unknown mints.

**Request:**
```json
{
  "method": "receive_cashu",
  "params": { "token": "cashuAeyJ0b2tlbiI6W3..." }
}
```

**Response:**
```json
{
  "amount": 1000,
  "mint_url": "https://mint.example.com"
}
```

**Process:**
1. Parse token (format: `cashu` + base64-encoded JSON)
2. Extract mint URL
3. Add mint if unknown
4. Validate and swap proofs with mint
5. Store in wallet database

#### `pay_cashu_request`

Pays a NUT-18 payment request. Returns token if no transport defined.

**Request:**
```json
{
  "method": "pay_cashu_request",
  "params": {
    "payment_request": "creqA...",
    "amount": 5000  // Optional: override amount
  }
}
```

**Response (no transport):**
```json
{
  "amount": 5000,
  "token": "cashuAeyJ0b2tlbiI6W3..."
}
```

**Response (with transport):**
```json
{
  "amount": 5000
}
```

**Behavior:**
- Parses NUT-18 request (format: `creq` + base64-encoded JSON)
- Selects compatible mint from wallet
- If transport defined (Nostr/HTTP): Sends token via transport
- If no transport: Returns token for manual delivery
- Supports amount-less requests with `amount` parameter

## Budget Management

Each connection has a spending budget:

```json
{
  "renewal_period": "daily|weekly|monthly|yearly|never",
  "renews_at": 1234567890,
  "total_budget_msats": 1000000,
  "used_budget_msats": 250000
}
```

**Enforcement:**
1. Check: `amount <= (total_budget_msats - used_budget_msats)`
2. Execute payment
3. Update: `used_budget_msats += amount`
4. Persist to storage

**Renewal:** Automatic when `current_time >= renews_at`

**Errors:** Returns `QUOTA_EXCEEDED` if budget insufficient

## Multi-Mint Architecture

Wallets manage multiple Cashu mints simultaneously:

- Each mint has its own wallet instance and SQLite database
- One mint is designated as "default" for Lightning operations
- Balances are aggregated across all mints
- Unknown mints are auto-added when receiving tokens

**Mint Selection:**
- **Receive**: Mint URL extracted from token (auto-add if needed)
- **Payment requests**: Match wallet mints against request's mint list
- **Lightning**: Uses default mint
- **Balance**: Aggregate across all mints

## Error Codes

### Standard NIP-47 Errors

`RATE_LIMITED`, `NOT_IMPLEMENTED`, `INSUFFICIENT_BALANCE`, `QUOTA_EXCEEDED`, `RESTRICTED`, `UNAUTHORIZED`, `INTERNAL`, `OTHER`

### CWC-Specific Errors

| Code | Description |
|------|-------------|
| `INVALID_TOKEN` | Malformed Cashu token |
| `MINT_ERROR` | Mint communication failure |
| `ALREADY_SPENT` | Proofs already redeemed |
| `INVALID_PAYMENT_REQUEST` | Malformed NUT-18 request |
| `NO_COMPATIBLE_MINT` | No matching mint |
| `AMOUNT_REQUIRED` | Amount-less request needs amount param |

## Info Event

Kind `13194` event announces supported methods:

```json
{
  "kind": 13194,
  "content": "get_balance make_invoice pay_invoice receive_cashu pay_cashu_request"
}
```

## Security Considerations

**Key Management:**
- Service keys derived from BIP39 mnemonic
- Unique keypair per connection
- NIP-04 encryption for all messages
- TODO: Move from plaintext storage to platform keychain

**Budget Enforcement:**
- Pre-payment check (not atomic)
- Concurrent requests may exceed budget
- Consider request queueing for strict enforcement

**Mint Trust:**
- Mints have custody of ecash
- Multi-mint diversification recommended
- Mint downtime affects operations

**Connection Security:**
- Unique keys limit compromise blast radius
- Budget caps damage from compromised connections
- Connections are revocable

## Implementation Notes

**Database:**
- NWC connections in SQLite
- Per-mint wallet databases (CDK `WalletSqliteDatabase`)
- Wallet seed in JSON (TODO: use keychain)

**Relays:**
- Default: `wss://nostrue.com`
- Local testing: `ws://localhost:4869`

**Storage Paths:**
- Connections: `~/Library/Application Support/Tollgate/TollgateApp/nwc-connections.sqlite`
- Wallets: `~/Library/Application Support/Tollgate/TollgateApp/wallets/<mint_hash>.sqlite`
- Seed: `~/Library/Application Support/Tollgate/TollgateApp/wallet-secrets.json`

## Example Flows

### Receiving Cashu Tokens

```
App → kind 23194: { "method": "receive_cashu", "params": { "token": "cashuA..." } }
Wallet → Parses token → Auto-adds mint if needed → Validates & swaps proofs
Wallet → kind 23195: { "amount": 1000, "mint_url": "https://mint.example.com" }
```

### Paying NUT-18 Request (No Transport)

```
App → kind 23194: { "method": "pay_cashu_request", "params": { "payment_request": "creqA..." } }
Wallet → Decodes request → Selects mint → Creates proofs → No transport = return token
Wallet → kind 23195: { "amount": 500, "token": "cashuA..." }
App → Delivers token out-of-band
```

### Creating Connection

```
User → Clicks "Create Connection"
Wallet → Generates keypair + budget → Creates URI
User → Copies URI: nostr+walletconnect://pubkey?relay=wss://nostrue.com&secret=hex
User → Pastes into app
App → Subscribes to kind 13194 → Begins sending requests
```

## Compatibility

**NIP-47 Compatible:**
- Event kinds (23194, 23195, 13194)
- NIP-04 encryption
- Connection URI format
- Core methods (get_balance, make_invoice, pay_invoice)

**Cashu NUTs:**
- NUT-00 (tokens), NUT-01 (keys), NUT-02 (keysets)
- NUT-03 (swap), NUT-04/05 (mint/melt Lightning)
- NUT-18 (payment requests)

## References

- [NIP-47: Nostr Wallet Connect](https://github.com/nostr-protocol/nips/blob/master/47.md)
- [NIP-04: Encrypted Direct Messages](https://github.com/nostr-protocol/nips/blob/master/04.md)
- [Cashu Protocol (NUTs)](https://github.com/cashubtc/nuts)
- [NUT-18: Payment Requests](https://github.com/cashubtc/nuts/blob/main/18.md)
- [NWC Documentation](https://docs.nwc.dev/)

## License

This specification is released under the MIT License.

---

**Changelog:**
- 2024-01-08: Initial draft (v0.1.0)

