import { invoke } from "@tauri-apps/api/core";

export type WalletBalance = {
  mint_url: string;
  balance: number;
  unit: string;
  pending: number;
};

export type Nut18PaymentRequestInfo = {
  request: string;
  amount: number | null;
  unit: string;
  description: string | null;
  mints: string[];
};

export type Bolt11InvoiceInfo = {
  quote_id: string;
  request: string;
  amount: number | null;
  unit: string;
  expiry: number;
  mint_url: string;
};

export type Bolt11PaymentResult = {
  amount: number;
  fee_paid: number;
  preimage: string | null;
};

export type WalletSummary = {
  total: number;
  default_mint: string | null;
  balances: WalletBalance[];
  npub: string | null;
};

export type WalletTransactionEntry = {
  id: string;
  direction: "incoming" | "outgoing";
  amount: number;
  fee: number;
  unit: string;
  timestamp: number;
  mint_url: string;
  memo: string | null;
  quote_id: string | null;
};

export type SwapRequest = {
        amount: number | null;
    amount_split_target: number | [number];
    proofs: [string];
    spending_conditions: [string]| null;
    include_fees: boolean;
};

export async function fetchWalletSummary(): Promise<WalletSummary> {
  return invoke<WalletSummary>("get_wallet_summary");
}

export async function fetchWalletTransactions(): Promise<WalletTransactionEntry[]> {
  return invoke<WalletTransactionEntry[]>("list_wallet_transactions");
}

export async function createNut18PaymentRequest(
  amount: number | null,
  description: string | null,
): Promise<Nut18PaymentRequestInfo> {
  return invoke<Nut18PaymentRequestInfo>("create_nut18_payment_request", {
    amount,
    description,
  });
}

export async function createBolt11Invoice(
  amount: number,
  description: string | null,
): Promise<Bolt11InvoiceInfo> {
  return invoke<Bolt11InvoiceInfo>("create_bolt11_invoice", {
    amount,
    description,
  });
}

export async function payNut18PaymentRequest(
  request: string,
  customAmount: number | null,
): Promise<void> {
  await invoke("pay_nut18_payment_request", {
    request,
    customAmount,
  });
}

export async function payBolt11Invoice(invoice: string): Promise<Bolt11PaymentResult> {
  return invoke<Bolt11PaymentResult>("pay_bolt11_invoice", {
    invoice,
  });
}

export async function receiveCashuToken(token: string): Promise<{ amount: number; mint_url: string }> {
  return invoke<{ amount: number; mint_url: string }>("receive_cashu_token", {
    token,
  });
}

export async function addMint(mintUrl: string): Promise<void> {
  await invoke("add_mint", {
    mintUrl,
  });
}

export async function removeMint(mintUrl: string): Promise<void> {
  await invoke("remove_mint", {
    mintUrl,
  });
}
