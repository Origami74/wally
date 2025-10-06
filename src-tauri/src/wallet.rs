use crate::{
    tollgate::wallet::{
        Bolt11InvoiceInfo, Bolt11PaymentResult, Nut18PaymentRequestInfo, WalletSummary,
        WalletTransactionEntry,
    },
    TollGateState,
};
use tauri::State;

#[tauri::command]
pub async fn add_mint(mint_url: String, state: State<'_, TollGateState>) -> Result<(), String> {
    let service = state.lock().await;
    service.add_mint(&mint_url).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_wallet_balance(state: State<'_, TollGateState>) -> Result<u64, String> {
    let service = state.lock().await;
    service
        .get_wallet_balance()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_nut18_payment_request(
    amount: Option<u64>,
    description: Option<String>,
    state: State<'_, TollGateState>,
) -> Result<Nut18PaymentRequestInfo, String> {
    let service = state.lock().await;
    service
        .create_nut18_payment_request(amount, description)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_bolt11_invoice(
    amount: u64,
    description: Option<String>,
    state: State<'_, TollGateState>,
) -> Result<Bolt11InvoiceInfo, String> {
    let service = state.lock().await;
    service
        .create_bolt11_invoice(amount, description)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pay_nut18_payment_request(
    request: String,
    custom_amount: Option<u64>,
    state: State<'_, TollGateState>,
) -> Result<(), String> {
    let service = state.lock().await;
    service
        .pay_nut18_payment_request(&request, custom_amount)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pay_bolt11_invoice(
    invoice: String,
    state: State<'_, TollGateState>,
) -> Result<Bolt11PaymentResult, String> {
    let service = state.lock().await;
    service
        .pay_bolt11_invoice(&invoice)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_wallet_summary(state: State<'_, TollGateState>) -> Result<WalletSummary, String> {
    let service = state.lock().await;
    service
        .get_wallet_summary()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_wallet_transactions(
    state: State<'_, TollGateState>,
) -> Result<Vec<WalletTransactionEntry>, String> {
    let service = state.lock().await;
    service
        .list_wallet_transactions()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn receive_cashu_token(
    token: String,
    state: State<'_, TollGateState>,
) -> Result<serde_json::Value, String> {
    let service = state.lock().await;
    match service.receive_cashu_token(&token).await {
        Ok(result) => Ok(serde_json::json!({
            "amount": result.amount,
            "mint_url": result.mint_url,
        })),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn create_external_token(
    amount_sats: u64,
    mint_url: Option<String>,
    state: State<'_, TollGateState>,
) -> Result<String, String> {
    let service = state.lock().await;
    service
        .create_external_token(amount_sats, mint_url)
        .await
        .map_err(|e| e.to_string())
}
