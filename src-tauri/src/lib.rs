use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;

use tauri::{Manager, State};
use tauri_plugin_log::{Target, TargetKind};

use cdk::nuts::{CurrencyUnit, MintQuoteState};
use cdk::wallet::MintQuote;
use cdk::Amount;
use cdk::{cdk_database::WalletMemoryDatabase, wallet::Wallet};

#[derive(Default)]
struct WalletState {
    wallet: Option<Wallet>,
    quote: Option<MintQuote>,
}

#[tauri::command]
fn create_wallet(
    mint_url: &str,
    seed: [u8; 32],
    state: State<'_, Mutex<WalletState>>,
) -> Result<String, String> {
    let mut state = state.lock().unwrap();

    match state.wallet {
        Some(_) => Err("Wallet already exist".to_owned()),
        None => {
            let localstore = WalletMemoryDatabase::default();
            match Wallet::new(
                mint_url,
                CurrencyUnit::Sat,
                Arc::new(localstore),
                &seed,
                None,
            ) {
                Ok(wallet) => {
                    state.wallet = Some(wallet);
                }
                Err(err) => {
                    return Err(err.to_string());
                }
            }
            Ok("success".to_owned())
        }
    }
}

use async_std::task;

#[tauri::command]
fn load_wallet_request(amount: u64, state: State<'_, Mutex<WalletState>>) -> Result<String, String> {
    let state = state.lock().unwrap();
    match &state.wallet {
        None => Err("Wallet does not exist".to_owned()),
        Some(wallet) => {
            let quote = task::block_on(wallet.mint_quote(Amount::from(amount), None)).unwrap();
            Ok(quote.request)
        }
    }
}

#[tauri::command]
fn load_wallet_finalise(amount: u64, state: State<'_, Mutex<WalletState>>) -> Result<String, String> {
    let mut state = state.lock().unwrap();
    match &state.wallet {
        None => Err("Wallet does not exist".to_owned()),
        Some(wallet) => {
            let quote = task::block_on(wallet.mint_quote(Amount::from(amount), None)).unwrap();
            let timeout = Duration::from_secs(90);
            let start = std::time::Instant::now();

            loop {
                let status = task::block_on(wallet.mint_quote_state(&quote.id));
                let status = status.unwrap();

                if status.state == MintQuoteState::Paid {
                    break;
                }

                if start.elapsed() >= timeout {
                    eprintln!("Timeout while waiting for mint quote to be paid");
                    return Err("Timeout while waiting for mint quote to be paid".to_owned());
                }

                println!("Quote state: {}", status.state);

                sleep(Duration::from_secs(2));
            }

            state.quote = Some(quote);
            Ok("success".to_owned())
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            app.manage(Mutex::new(WalletState::default()));
            Ok(())
        })
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets([
                    Target::new(TargetKind::Stdout),
                    Target::new(TargetKind::Webview),
                ])
                .build(),
        )
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_androidwifi::init())
        .invoke_handler(tauri::generate_handler![create_wallet, load_wallet_request, load_wallet_finalise])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
