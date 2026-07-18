# PR #20 review — mint fallback

Reviewed `Origami74/wally#20` at `4994233cb8dd94b15a2810698b5d6f981f803a19` and compared its paid proxy flow with `Routstr/routstrd@mint-fallback`.

## Findings addressed on `mint-fallback`

1. **Payment unit mismatch** — `gateway.rs` passed msats to an API named and used as sats. Sat mints could overpay by 1000×, while msat mints relied on the accidental mismatch. The gateway now rounds msats up to sats, and the wallet converts sats to the selected wallet's native unit.
2. **Payment creation failed open** — wallet errors were discarded with `.ok()`, sending paid requests without `X-Cashu`. Required payments now fail closed with HTTP 402.
3. **No `mint_unreachable` fallback** — the proxy attempted one mint once. It now recognizes the provider's nested `mint_unreachable` server error, reclaims the unaccepted token, excludes the failed mint, creates a token from the next deterministic funded mint, and retries with bounded attempts.
4. **Sensitive token logging** — full response headers and Cashu change tokens were logged. Token/header values are no longer logged; wallet messages use a consistent `[wallet]` prefix.
5. **Incorrect success classification** — only `200 OK` counted as success. All 2xx responses now use `status.is_success()`.
6. **Mint keyset typo** — sat keysets were checked as `ssat`; corrected to `sat`.

## Not issues (checked)

- Tauri wallet state registration matches gateway lookups.
- Routstr storage is isolated under the application data directory.
- Refund handling does not hold Routstr and wallet mutexes simultaneously.
- Fallback attempts are deduplicated, deterministic, exclude failed mints, and only trigger for server-side `mint_unreachable` errors.
- Unrelated provider errors do not trigger request replay.

## Verification

- `cargo fmt --all`
- `cargo check --tests`
- `cargo test --lib` — 17 passed, 0 failed
- `git diff --check`
