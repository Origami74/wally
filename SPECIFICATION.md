# Cashu Wallet Connect (CWC) Specification

**Version:** 0.1.0  
**Status:** Draft  
**Authors:** Wally Contributors  
**Based on:** [NIP-47 (Nostr Wallet Connect)](https://github.com/nostr-protocol/nips/blob/master/47.md)

## Abstract

Cashu Wallet Connect (CWC) is an extension of the Nostr Wallet Connect (NWC) protocol that enables applications to interact with Cashu ecash wallets over Nostr. CWC maintains compatibility with the core NIP-47 protocol while adding Cashu-specific payment methods, supporting both Lightning Network (via mint gateways) and native Cashu token operations.

## Motivation

While NWC provides an excellent protocol for Lightning wallet integration, the growing Cashu ecosystem requires:

1. **Native ecash support**: Direct handling of Cashu tokens without requiring Lightning infrastructure
2. **Multi-mint architecture**: Managing balances across multiple Cashu mints
3. **Privacy-preserving payments**: Leveraging Cashu's privacy properties
4. **Offline token handling**: Supporting ecash tokens that can be held and transferred offline
5. **Bridge functionality**: Interoperability between Cashu and Lightning networks

CWC addresses these needs by extending NIP-47 with Cashu-specific operations while maintaining backward compatibility where possible.

## Protocol Overview

### Base Protocol

CWC uses [NIP-47 (Nostr Wallet Connect)](https://github.com/nostr-protocol/nips/blob/master/47.md) as its foundation:

- **Request events**: Kind `23194` (WalletConnectRequest)
- **Response events**: Kind `23195` (WalletConnectResponse)
- **Info events**: Kind `13194` (WalletConnectInfo)
- **Encryption**: NIP-04 encryption for all request/response content
- **Transport**: Nostr relays for event distribution

### Key Differences from Standard NWC

1. **Backend**: Connects to Cashu mints instead of Lightning nodes
2. **Primary assets**: Cashu tokens (ecash proofs) with Lightning as secondary via mint gateways
3. **Multi-mint support**: Manages multiple mint URLs simultaneously
4. **Extended methods**: Additional methods for Cashu-specific operations
5. **Balance representation**: Aggregates balances across multiple mints

## Connection Types

CWC supports two connection modes, both inherited from NIP-47:

### Standard NWC Connection

1. Wallet generates a unique keypair for the connection
2. Creates connection URI: `nostr+walletconnect://<service_pubkey>?relay=<relay_url>&secret=<connection_secret>`
3. App uses the secret to sign requests FROM the connection keypair TO the service pubkey
4. Service decrypts using the connection's pubkey

**Connection URI Format:**
```
nostr+walletconnect://<hex-pubkey>?relay=<relay-url>&secret=<hex-secret>[&lud16=<lud16>]
```

### Nostr Wallet Auth (NWA) Connection

1. App generates its own keypair
2. App makes authorization request with its pubkey
3. Wallet generates a unique connection keypair
4. On approval, wallet broadcasts kind `33194` event with connection details
5. App sends requests FROM its own keypair TO the connection pubkey
6. Service decrypts using the app's pubkey

**NWA Flow:**
```
App → HTTP POST to wallet
Wallet → User approval prompt
Wallet → Kind 33194 approval event (encrypted for app)
App → Kind 23194 requests (signed by app)
```

## Methods

### Standard NIP-47 Methods

#### `get_balance`

Returns the total balance across all configured mints.

**Request:**
```json
{
  "method": "get_balance"
}
```

**Response:**
```json
{
  "result_type": "get_balance",
  "result": {
    "balance": 50000
  }
}
```

**CWC Extensions:**

The response includes additional fields specific to multi-mint architecture:

```json
{
  "result_type": "get_balance",
  "result": {
    "balance": 50000,
    "max_sendable": 30000,
    "mints": [
      {
        "mint_url": "https://mint1.example.com",
        "balance": 30000,
        "unit": "msat"
      },
      {
        "mint_url": "https://mint2.example.com",
        "balance": 20000,
        "unit": "msat"
      }
    ]
  }
}
```

- `balance`: Total balance in millisatoshis across all mints
- `max_sendable`: Maximum single payment amount (limited by largest mint balance)
- `mints`: Array of per-mint balance information

#### `make_invoice`

Creates a BOLT11 Lightning invoice via the default mint's Lightning gateway.

**Request:**
```json
{
  "method": "make_invoice",
  "params": {
    "amount": 10000,
    "description": "Payment description"
  }
}
```

**Response:**
```json
{
  "result_type": "make_invoice",
  "result": {
    "invoice": "lnbc100n1...",
    "payment_hash": "abc123..."
  }
}
```

**Parameters:**
- `amount`: Amount in millisatoshis
- `description`: Optional invoice description

**Notes:**
- Uses the wallet's default mint
- Mint must support Lightning gateway functionality (NUT-04, NUT-05)
- Invoice creation generates a mint quote internally

#### `pay_invoice`

Pays a BOLT11 Lightning invoice using Cashu tokens from the default mint.

**Request:**
```json
{
  "method": "pay_invoice",
  "params": {
    "invoice": "lnbc100n1..."
  }
}
```

**Response:**
```json
{
  "result_type": "pay_invoice",
  "result": {
    "preimage": "def456...",
    "fees_paid": 100
  }
}
```

**Process:**
1. Parse invoice and extract amount
2. Check connection budget
3. Request melt quote from mint
4. Execute payment (burn tokens, receive preimage)
5. Update budget usage

**Budget Check:**
- Payment amount must not exceed remaining connection budget
- Returns `QUOTA_EXCEEDED` error if budget insufficient

### Cashu-Specific Methods

#### `receive_cashu`

Receives and validates a Cashu token, adding it to the wallet.

**Request:**
```json
{
  "method": "receive_cashu",
  "params": {
    "token": "cashuAeyJ0b2tlbiI6W3sicHJvb2ZzIjpb..."
  }
}
```

**Response:**
```json
{
  "result_type": "receive_cashu",
  "result": {
    "amount": 1000,
    "mint_url": "https://mint.example.com"
  }
}
```

**Process:**
1. Parse Cashu token (base64-encoded JSON)
2. Extract mint URL from token
3. Auto-add mint to wallet if not already configured
4. Validate proofs with mint
5. Swap proofs for fresh proofs (re-blinding)
6. Store proofs in wallet database
7. Return received amount and mint URL

**Token Format:**

Cashu tokens follow the [Cashu token format](https://github.com/cashubtc/nuts/blob/main/00.md):

```json
{
  "token": [
    {
      "mint": "https://mint.example.com",
      "proofs": [
        {
          "id": "JHV8eUnoAln/",
          "amount": 1,
          "secret": "...",
          "C": "02..."
        }
      ]
    }
  ]
}
```

Encoded as: `cashu` + base64(json)

**Error Handling:**
- `INVALID_TOKEN`: Token parse failure
- `MINT_ERROR`: Mint validation failure  
- `ALREADY_SPENT`: Proofs already redeemed

#### `pay_cashu_request`

Pays a NUT-18 payment request, either via transport or returning a token.

**Request:**
```json
{
  "method": "pay_cashu_request",
  "params": {
    "payment_request": "creqA...",
    "amount": 5000
  }
}
```

**Response (with transport):**
```json
{
  "result_type": "pay_cashu_request",
  "result": {
    "amount": 5000
  }
}
```

**Response (no transport):**
```json
{
  "result_type": "pay_cashu_request",
  "result": {
    "amount": 5000,
    "token": "cashuAeyJ0b2tlbiI6W3sicHJvb2ZzIjpb..."
  }
}
```

**Parameters:**
- `payment_request`: Base64-encoded NUT-18 payment request
- `amount`: Optional amount override (required if payment request is amount-less)

**NUT-18 Payment Request Structure:**

```json
{
  "at": {
    "type": "nostr",
    "n": "nprofile1..."
  },
  "a": 5,
  "i": "b17",
  "u": "sat",
  "m": ["https://mint.example.com"]
}
```

Encoded as: `creq` + base64(json)

**Process:**

1. Parse and decode payment request
2. Determine amount (from request or override parameter)
3. Select compatible mint from wallet (must match request's mint list)
4. Check balance availability
5. Generate proofs of specified amount
6. If transport defined (Nostr/HTTP): Send via transport
7. If no transport: Return token for out-of-band delivery

**Transport Types:**
- **Nostr (`at.type: "nostr"`)**: Posts token to recipient's Nostr pubkey
- **HTTP (`at.type: "http"`)**: POSTs token to specified URL
- **None**: Returns token in response for manual delivery

## Budget Management

Each connection has an associated budget to limit spending:

### Budget Structure

```json
{
  "renewal_period": "daily" | "weekly" | "monthly" | "yearly" | "never",
  "renews_at": 1234567890,
  "total_budget_msats": 1000000,
  "used_budget_msats": 250000
}
```

**Fields:**
- `renewal_period`: How often the budget resets
- `renews_at`: Unix timestamp of next renewal
- `total_budget_msats`: Maximum spending limit
- `used_budget_msats`: Amount spent in current period

### Budget Enforcement

1. **Pre-payment check**: Verify `amount <= (total_budget_msats - used_budget_msats)`
2. **Payment execution**: Perform payment operation
3. **Post-payment update**: Increment `used_budget_msats` by payment amount
4. **Persistence**: Save updated budget to storage

### Budget Renewal

Automatic renewal occurs when `current_time >= renews_at`:

1. Reset `used_budget_msats` to 0
2. Calculate new `renews_at` based on `renewal_period`
3. Persist updated budget

**Renewal Calculation:**
- Daily: `renews_at = current_time + 86400`
- Weekly: `renews_at = current_time + 604800`
- Monthly: `renews_at = current_time + 2592000` (30 days)
- Yearly: `renews_at = current_time + 31536000` (365 days)

### Budget Errors

**Error Code:** `QUOTA_EXCEEDED`

Returned when a payment would exceed the remaining budget:

```json
{
  "result_type": "pay_invoice",
  "error": {
    "code": "QUOTA_EXCEEDED",
    "message": "Payment amount exceeds remaining budget"
  }
}
```

## Multi-Mint Architecture

### Mint Management

Wallets maintain a collection of mints, each with:

- **Mint URL**: Unique identifier (e.g., `https://mint.example.com`)
- **Wallet instance**: Separate CDK wallet per mint
- **Database**: Per-mint SQLite database for proofs and transactions
- **Keys**: Cached keyset information

### Default Mint

One mint is designated as the "default" for:
- Creating BOLT11 invoices (`make_invoice`)
- Paying BOLT11 invoices (`pay_invoice`)
- Operations that don't specify a mint

### Mint Selection

**For receiving tokens:**
- Automatic: Mint URL extracted from token
- Auto-adds mint if not already configured

**For payment requests:**
- Matches wallet's mints against payment request's mint list
- Selects first matching mint
- Falls back to default mint if no match

**For balance queries:**
- Aggregates across all configured mints
- Returns per-mint breakdown

### Mint Discovery

When receiving a token from an unknown mint:

1. Parse token to extract mint URL
2. Check if mint already in wallet
3. If not present:
   - Create new wallet instance for mint
   - Initialize per-mint database
   - Fetch mint info and keysets
   - Add to wallet's mint collection
   - Persist configuration
4. Proceed with token reception

## Error Codes

CWC uses standard NIP-47 error codes plus Cashu-specific codes:

### Standard NIP-47 Errors

| Code | Description |
|------|-------------|
| `RATE_LIMITED` | Too many requests |
| `NOT_IMPLEMENTED` | Method not supported |
| `INSUFFICIENT_BALANCE` | Wallet balance too low |
| `QUOTA_EXCEEDED` | Budget limit reached |
| `RESTRICTED` | Method not permitted |
| `UNAUTHORIZED` | Invalid credentials |
| `INTERNAL` | Server error |
| `OTHER` | Other error |

### CWC-Specific Errors

| Code | Description |
|------|-------------|
| `INVALID_TOKEN` | Malformed Cashu token |
| `MINT_ERROR` | Mint communication failure |
| `ALREADY_SPENT` | Token proofs already redeemed |
| `INVALID_PAYMENT_REQUEST` | Malformed NUT-18 request |
| `NO_COMPATIBLE_MINT` | No matching mint for payment |
| `MINT_NOT_FOUND` | Specified mint not configured |
| `AMOUNT_REQUIRED` | Amount-less request needs amount param |

### Error Response Format

```json
{
  "result_type": "method_name",
  "error": {
    "code": "ERROR_CODE",
    "message": "Human-readable error description"
  }
}
```

## Info Event

Wallets broadcast a kind `13194` info event announcing their capabilities:

```json
{
  "kind": 13194,
  "content": "get_balance make_invoice pay_invoice receive_cashu pay_cashu_request",
  "pubkey": "wallet_service_pubkey",
  "created_at": 1234567890,
  "tags": []
}
```

The `content` field lists supported methods as a space-separated string.

**CWC Standard Methods:**
```
get_balance make_invoice pay_invoice receive_cashu pay_cashu_request
```

## Security Considerations

### Key Management

1. **Service Keys**: Derived from wallet's BIP39 mnemonic
2. **Connection Keys**: Unique keypair per connection
3. **Encryption**: NIP-04 for all request/response encryption
4. **Storage**: Keys stored in platform secure storage (TODO: current implementation uses plaintext)

### Budget Limits

Budget enforcement provides spending limits but has limitations:

- **Pre-check only**: Budget checked before payment, not atomically locked
- **Race conditions**: Concurrent requests could exceed budget
- **Mitigation**: Consider request queueing for strict enforcement

### Mint Trust

Users must trust configured mints:

- **Custody**: Mints hold custody of ecash proofs
- **Privacy**: Mints can track token patterns (though blind signatures provide privacy)
- **Availability**: Mint downtime affects wallet operations
- **Mitigation**: Multi-mint diversification reduces single-point risk

### Connection Security

- **Unique keys per connection**: Limits blast radius of key compromise
- **Budget limits**: Caps damage from compromised connections
- **Revocable**: Connections can be deleted without affecting other connections
- **Audit**: Connection list visible to user

## Implementation Notes

### Database Schema

**NWC Connections:**
```sql
CREATE TABLE connections (
    id INTEGER PRIMARY KEY,
    connection_secret TEXT NOT NULL UNIQUE,
    connection_pubkey TEXT NOT NULL,
    app_pubkey TEXT,
    secret TEXT,
    renewal_period TEXT NOT NULL,
    renews_at INTEGER,
    total_budget_msats INTEGER NOT NULL,
    used_budget_msats INTEGER NOT NULL,
    name TEXT,
    created_at INTEGER NOT NULL
);
```

**Per-Mint Wallets:**
- Managed by CDK's `WalletSqliteDatabase`
- Stores proofs, transactions, keysets
- Separate database per mint URL

### Relay Selection

**Default Relay:** `wss://nostrue.com`

**Local Relay Option:** `ws://localhost:4869` (for testing/privacy)

Connections can specify different relays via connection URI parameter.

### Persistence

**Wallet Seed:**
- Location: `~/Library/Application Support/Tollgate/TollgateApp/wallet-secrets.json`
- Format: BIP39 mnemonic (12 words)
- TODO: Move to platform keychain

**NWC Connections:**
- Location: `~/Library/Application Support/Tollgate/TollgateApp/nwc-connections.sqlite`
- SQLite database

**Mint Wallets:**
- Location: `~/Library/Application Support/Tollgate/TollgateApp/wallets/`
- Filename: `<mint_hash>.sqlite`

## Example Flows

### Flow 1: Receiving Cashu Tokens

```
1. App sends NIP-04 encrypted event (kind 23194):
   {
     "method": "receive_cashu",
     "params": {
       "token": "cashuAeyJ0b2tlbiI6W3sicHJvb2..."
     }
   }

2. Wallet receives and processes:
   - Decrypts request
   - Parses token
   - Extracts mint URL: "https://mint.example.com"
   - Checks if mint exists (if not, adds it)
   - Validates proofs with mint
   - Swaps proofs for fresh proofs
   - Stores in database

3. Wallet responds with encrypted event (kind 23195):
   {
     "result_type": "receive_cashu",
     "result": {
       "amount": 1000,
       "mint_url": "https://mint.example.com"
     }
   }
```

### Flow 2: Paying NUT-18 Request (No Transport)

```
1. App creates payment request:
   payment_request = {
     "a": 500,
     "u": "sat",
     "m": ["https://mint.example.com"]
   }
   encoded = "creq" + base64(payment_request)

2. App sends request:
   {
     "method": "pay_cashu_request",
     "params": {
       "payment_request": "creqA..."
     }
   }

3. Wallet processes:
   - Decodes payment request
   - Checks amount (500 sats)
   - Finds compatible mint
   - Checks balance: 1000 sats available ✓
   - Creates proofs worth 500 sats
   - No transport defined → returns token

4. Wallet responds:
   {
     "result_type": "pay_cashu_request",
     "result": {
       "amount": 500,
       "token": "cashuAeyJ0b2tlbiI6W3..."
     }
   }

5. App delivers token to recipient out-of-band
```

### Flow 3: Creating Standard NWC Connection

```
1. User clicks "Create Connection" in wallet UI

2. Wallet generates:
   - New keypair for connection
   - Default budget (1000 sats, daily renewal)

3. Wallet creates URI:
   nostr+walletconnect://service_pubkey_hex?relay=wss://nostrue.com&secret=connection_secret_hex

4. User copies URI and pastes into app

5. App parses URI and connects:
   - Subscribes to kind 13194 from service_pubkey
   - Verifies supported methods
   - Begins sending kind 23194 requests
```

## Compatibility

### NIP-47 Compatibility

**Compatible:**
- ✅ Event kinds (23194, 23195, 13194)
- ✅ NIP-04 encryption
- ✅ Connection URI format
- ✅ get_balance, make_invoice, pay_invoice methods

**Extended:**
- ➕ Additional methods (receive_cashu, pay_cashu_request)
- ➕ Extended balance response (multi-mint data)

**Not Implemented:**
- ❌ get_info
- ❌ multi_pay_invoice
- ❌ pay_keysend
- ❌ lookup_invoice
- ❌ list_transactions (available as separate wallet API)

### Cashu NUT Compatibility

**Implemented:**
- ✅ NUT-00: Token format
- ✅ NUT-01: Mint keys
- ✅ NUT-02: Keysets
- ✅ NUT-03: Swap
- ✅ NUT-04: Mint tokens (Lightning deposit)
- ✅ NUT-05: Melt tokens (Lightning withdrawal)
- ✅ NUT-18: Payment requests

## Future Enhancements

### Planned Features

1. **Multi-path payments**: Combine tokens from multiple mints
2. **Peer-to-peer swap**: Direct mint-to-mint swaps
3. **Subscription support**: Recurring payment authorization
4. **Transaction history**: Implement list_transactions
5. **Proof management**: Selective proof deletion, consolidation
6. **Multi-currency**: Support non-sat units

### Under Consideration

1. **Fedimint integration**: Support for Fedimint ecash
2. **Atomic swaps**: Trustless mint-to-mint exchanges
3. **Privacy enhancements**: Additional blinding layers
4. **Mobile optimizations**: Bandwidth-efficient protocols

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

