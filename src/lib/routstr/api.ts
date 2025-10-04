import { invoke } from "@tauri-apps/api/core";
import type {
  RoutstrModel,
  RoutstrConnectionStatus,
  RoutstrWalletBalance,
  RoutstrTopUpResponse,
  RoutstrRefundResponse,
  RoutstrCreateResponse
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

export async function setRoutstrApiKey(apiKey: string): Promise<void> {
  return invoke("routstr_set_api_key", { apiKey });
}

export async function createRoutstrWallet(url: string, cashuToken: string): Promise<RoutstrCreateResponse> {
  return invoke("routstr_create_wallet", { url, cashuToken });
}

export async function createBalanceWithToken(cashuToken: string): Promise<RoutstrCreateResponse> {
  return invoke("routstr_create_balance_with_token", { cashuToken });
}

export async function getRoutstrWalletBalance(): Promise<RoutstrWalletBalance> {
  return invoke("routstr_get_wallet_balance");
}

export async function topUpRoutstrWallet(cashuToken: string): Promise<RoutstrTopUpResponse> {
  return invoke("routstr_top_up_wallet", { cashuToken });
}

export async function refundRoutstrWallet(): Promise<RoutstrRefundResponse> {
  return invoke("routstr_refund_wallet");
}

export async function setRoutstrAutoTopupConfig(
  enabled: boolean,
  minThreshold: number,
  targetAmount: number
): Promise<void> {
  return invoke("routstr_set_auto_topup_config", {
    enabled,
    minThreshold,
    targetAmount
  });
}

export async function getRoutstrAutoTopupConfig(): Promise<{
  enabled: boolean;
  min_threshold: number;
  target_amount: number;
}> {
  return invoke("routstr_get_auto_topup_config");
}

export async function getStoredRoutstrApiKey(): Promise<string | null> {
  return invoke("routstr_get_stored_api_key");
}

export async function clearRoutstrConfig(): Promise<void> {
  return invoke("routstr_clear_config");
}