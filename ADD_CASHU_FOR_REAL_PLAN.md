# ADD_CASHU_FOR_REAL_PLAN

## Objectives
- Update to the latest `cdk` APIs and wire them to Tollgate so that we can: (1) create Nut18 payment requests, (2) create standard BOLT11 invoices, (3) pay Nut18 requests, (4) pay BOLT11 invoices, (5) surface QR codes on receive flows, (6) manage a 12-word seed (or derive from user-provided nsec), (7) show live wallet balance, and (8) list transactions/history in the UI.

## Assumptions & Inputs Needed
- We will upgrade `cdk` to the most recent commit or crate release (verify change log for API breakage around `Wallet`, `Mint`, and Nut18 helpers).
- Secret material: either generate/safely store a BIP39 12-word seed, or accept/import a Nostr `nsec` and document how we derive the wallet seed from it. We need to decide at plan execution time.
- Each wallet instance requires: default mint URL, a Nostr npub for Nut18 requests, and a secret signing key. These values should ultimately live in secure storage per platform.
- The backend remains the single source of truth; React will always call Tauri commands for authoritative state.

## Phase 0 – Dependency & Build Prep
- Bump `cdk` git reference/version in `src-tauri/Cargo.toml`; run `cargo update -p cdk` and resolve any new transitive requirements (Pay attention to `cdk-lfs` feature flags, database backends, and breaking API changes like `Wallet::new` signature).
- Audit existing `tollgate::wallet` usage for API drift (e.g., `Wallet::send`, `MintQuote`, `SplitTarget`) and adjust imports/types to match the new release.
- Regenerate bindings/lockfiles (`cargo check`, `pnpm install`) to ensure both Rust and frontend still build before feature work.

## Phase 1 – Wallet Core & Secret Management
- Implement a secure seed bootstrapper:
  - Generate a BIP39 seed (12 words) on first launch, encrypt/store it (macOS: Keychain via `tauri-plugin-keychain` or secure file; Android: Keystore). Provide fallback path to import a user-supplied Nostr `nsec` if we choose that route.
  - On startup, resolve the seed -> 32-byte array used for `cdk::wallet::Wallet` construction.
- Persist derived public keys (npub) and mint URL(s) in our settings model so the frontend can render/edit them.
- Expose Tauri commands for retrieving and updating wallet configuration, including toggling between generated seed vs. provided nsec.

## Phase 2 – Wallet Persistence & Multi-Mint Support
- Swap the in-memory `WalletMemoryDatabase` for a persistent backend (SQLite via `WalletSqliteDatabase` or filesystem-backed DB) so balances, proofs, and history survive restarts on both desktop and Android.
- Extend `TollGateWallet` to manage multiple `Wallet` instances keyed by mint URL, with helpers to set/get the default mint.
- Add synchronization routines to refresh balances/transactions from each mint at startup and on demand (triggered via background task or explicit refresh command).
- Store any Nut18 commitment state that `cdk` requires (proof storage, pending requests) inside the same DB.

## Phase 3 – Receive Flow (Features 1, 2, 5)
- Implement backend helpers:
  - `create_nut18_payment_request(amount_sats, description?) -> Nut18PaymentRequest` using `cdk::cashu::nuts::nut18::PaymentRequestBuilder` (ensure we inject mint URL, npub, and secret key).
  - `create_bolt11_invoice(amount_sats, description?) -> Bolt11Invoice` leveraging `cdk`'s Lightning integration (connect to mint’s Lightning node or external LSP as required).
  - Return structured responses including payment request string, fallback text, expiry, and metadata needed by the frontend.
- Add Tauri commands wrapping these helpers and emit events (e.g., `wallet:invoice-created`) so the UI can react without polling.
- Surface QR payloads: return both the raw string and a `data:` URI (frontend can also handle QR generation via JS library such as `qrcode.react`).

## Phase 4 – Send Flow (Features 3, 4)
- Add backend functions:
  - `pay_nut18_request(request: String)` that parses, validates against available proofs, and spends tokens through `Wallet::receive` / `Wallet::pay_nut18` equivalents.
  - `pay_bolt11_invoice(invoice: String)` that buys proofs if needed and triggers Lightning payment via mint.
- Handle insufficient balance by optionally auto-minting (if the mint allows) or returning rich errors for the UI.
- Emit progress/status events (initiated, awaiting mint, success/failure) so the frontend can show spinners and confirmations.
- Update session/tollgate logic to consume real payments where appropriate.

## Phase 5 – Balance & History (Features 7, 8)
- Expose a consolidated `get_wallet_balance` command that returns:
  - Total sats, per-mint breakdown, pending amounts, and last refresh time.
- Implement `list_transactions(limit?, cursor?)` returning transactions from `cdk`’s history (proof spends, reissues, Lightning payments, Nut18 receipts). Include metadata to classify entries for UI (incoming/outgoing, type, counterparties).
- Add a `history` route in the frontend:
  - Display paginated list with amount, status, timestamp, and type icons underneath settings icon per design instructions.
  - Provide navigation hook to return to home (same top-right button pattern).

## Phase 6 – Frontend Integration & UX Updates
- Expand the React query/service layer to wrap new Tauri commands (e.g., `useWalletBalance`, `usePaymentRequests`). Ensure hooks refresh when backend emits events.
- Update Receive screen to:
  - Accept amount input, call `create_*` commands, render QR + copy button state, show invoice string in a collapsible detail.
  - Add success/failure toasts and disable controls while requests are in-flight.
- Update Send screen to:
  - Parse clipboard/paste, auto-detect Nut18 vs Bolt11, call the matching Tauri command, show confirmation states.
  - Present errors inline with guidance (insufficient funds, expired invoice, etc.).
- Integrate balance widget on Home with real data and refresh indicators; consider live updates via event stream.
- Add new History icon/route and wire top-right header button logic: wallet ↔ settings ↔ history while keeping consistent padding/scroll behavior.

## Phase 7 – Background Services & Sync
- Ensure background tasks refresh wallet state on a cadence (pull new proofs, settle pending payments) without blocking UI.
- Handle platform-specific network changes (existing Tollgate detectors) so the wallet re-syncs when connectivity resumes.
- Consider debouncing simultaneous requests to avoid mint rate limits.

## Phase 8 – Testing, Instrumentation, and Documentation
- Add happy-path integration tests in Rust for each new command (using mocked mint where possible) plus unit tests for seed handling.
- Provide manual test checklist covering all eight target features on macOS and Android builds.
- Document new environment variables/secrets (where to set mint URL, nsec/seed) and update `README.md` with setup steps and key Tauri commands for QA.
- Capture follow-up TODOs: hooking real mint credentials, handling multi-nsec accounts, improving error surfaces.

## Open Questions / Follow-ups
- Decide whether we always derive the wallet seed from user-provided Nostr keys or allow generating a fresh wallet seed (and how to export it to users).
- Clarify Lightning connectivity: do we rely on the mint’s built-in Lightning backend or do we need our own LSP/LSAT integration?
- Confirm multiple mint support requirements (do we need to support selecting different mints per transaction?).
- Determine UX for seed backup/export and recovery flows before shipping to production.
