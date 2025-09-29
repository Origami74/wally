export type SessionStatusType = "Initializing" | "Active" | "Renewing" | "Expired" | "Error" | string;

export type PricingOption = {
  asset_type: string;
  price_per_step: number;
  price_unit: string;
  mint_url: string;
  min_steps: number;
};

export type TollgateAdvertisement = {
  metric: string;
  step_size: number;
  pricing_options: PricingOption[];
  tips: string[];
  tollgate_pubkey: string;
};

export type NetworkInfo = {
  gateway_ip: string;
  mac_address: string;
  is_tollgate: boolean;
  advertisement?: TollgateAdvertisement | null;
};

export type SessionInfo = {
  id: string;
  tollgate_pubkey: string;
  gateway_ip: string;
  status: SessionStatusType;
  usage_percentage: number;
  remaining_time_seconds: number | null;
  remaining_data_bytes: number | null;
  total_spent: number;
};

export type ServiceStatus = {
  auto_tollgate_enabled: boolean;
  current_network: NetworkInfo | null;
  active_sessions: SessionInfo[];
  wallet_balance: number;
  last_check: string;
};
