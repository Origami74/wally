export interface Architecture {
  modality: string;
  input_modalities: string[];
  output_modalities: string[];
  tokenizer: string;
  instruct_type: string | null;
}

export interface Pricing {
  prompt: number;
  completion: number;
  request: number;
  image: number;
  web_search: number;
  internal_reasoning: number;
  max_prompt_cost: number;
  max_completion_cost: number;
  max_cost: number;
}

export interface SatsPricing {
  prompt: number;
  completion: number;
  request: number;
  image: number;
  web_search: number;
  internal_reasoning: number;
  max_prompt_cost: number;
  max_completion_cost: number;
  max_cost: number;
}

export interface TopProvider {
  context_length: number;
  max_completion_tokens: number | null;
  is_moderated: boolean;
}

export interface RoutstrModel {
  id: string;
  name: string;
  created: number;
  description: string;
  context_length: number | null;
  architecture: Architecture | null;
  pricing: Pricing | null;
  sats_pricing: SatsPricing;
  per_request_limits: any | null;
  top_provider: TopProvider | null;
}

export interface RoutstrConnectionStatus {
  connected: boolean;
  base_url: string | null;
  model_count: number;
  has_api_key: boolean;
}

export interface RoutstrWalletBalance {
  api_key?: string;
  balance: number;
  reserved: number;
}

export interface RoutstrTopUpResponse {
  msats: number;
}

export interface RoutstrCreateResponse {
  api_key: string;
  balance: number;
}

export interface RoutstrRefundResponse {
  token?: string;
  recipient?: string;
  sats?: string;
  msats?: string;
}

export interface ApiKeyEntry {
  api_key: string;
  creation_cashu_token?: string;
  created_at: number;
  alias?: string;
}

export interface ProxyStatus {
  use_proxy: boolean;
  proxy_endpoint: string | null;
  target_service_url: string | null;
  use_onion: boolean;
  payment_required: boolean;
  cost_per_request_sats: number;
  use_manual_url: boolean;
  selected_provider_id: string | null;
  service_mode: string;
}

export interface WalletBalance {
  mint_url: string;
  balance: number;
  unit: string;
  pending: number;
}

export interface WalletSummary {
  total: number;
  default_mint: string | null;
  balances: WalletBalance[];
  npub: string | null;
}
