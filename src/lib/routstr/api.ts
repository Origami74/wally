import { invoke } from "@tauri-apps/api/core";
import type {
  RoutstrModel,
  RoutstrConnectionStatus,
  RoutstrWalletBalance,
  RoutstrTopUpResponse,
  RoutstrRefundResponse,
  RoutstrCreateResponse,
  ApiKeyEntry,
} from "./types";

export async function connectToRoutstrService(url: string): Promise<void> {
  return invoke("routstr_connect_service", { url });
}

export async function disconnectFromRoutstrService(): Promise<void> {
  return invoke("routstr_disconnect_service");
}

export async function refreshRoutstrModels(): Promise<void> {
  return invoke("routstr_refresh_models");
}

export async function getRoutstrModels(): Promise<RoutstrModel[]> {
  return invoke("routstr_get_models");
}

export async function getRoutstrConnectionStatus(): Promise<RoutstrConnectionStatus> {
  return invoke("routstr_get_connection_status");
}

export async function createRoutstrWallet(
  url: string,
  cashuToken: string,
): Promise<RoutstrCreateResponse> {
  return invoke("routstr_create_wallet", { url, cashuToken });
}

export async function createBalanceWithToken(
  cashuToken: string,
): Promise<RoutstrCreateResponse> {
  return invoke("routstr_create_balance_with_token", { cashuToken });
}


export async function clearRoutstrConfig(): Promise<void> {
  return invoke("routstr_clear_config");
}

export async function getAllApiKeys(): Promise<ApiKeyEntry[]> {
  return invoke("routstr_get_all_api_keys");
}

export async function getAllWalletBalances(): Promise<RoutstrWalletBalance[]> {
  return invoke("routstr_get_all_wallet_balances");
}

export async function getWalletBalanceForKey(
  apiKey: string,
): Promise<RoutstrWalletBalance> {
  return invoke("routstr_get_wallet_balance_for_key", { apiKey });
}

export async function topUpWalletForKey(
  apiKey: string,
  cashuToken: string,
): Promise<RoutstrTopUpResponse> {
  return invoke("routstr_top_up_wallet_for_key", { apiKey, cashuToken });
}

export async function refundWalletForKey(
  apiKey: string,
): Promise<RoutstrRefundResponse> {
  return invoke("routstr_refund_wallet_for_key", { apiKey });
}

export async function removeApiKey(apiKey: string): Promise<boolean> {
  return invoke("routstr_remove_api_key", { apiKey });
}

export async function forceResetAllApiKeys(): Promise<void> {
  return invoke("routstr_force_reset_all_api_keys");
}
