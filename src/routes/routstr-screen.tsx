import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import { Screen } from "@/components/layout/screen";
import { SectionHeader } from "@/components/layout/section-header";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { CopyButton } from "@/components/copy-button";
import type {
  RoutstrModel,
  RoutstrConnectionStatus,
  RoutstrWalletBalance,
  RoutstrAutoTopupConfig,
} from "@/lib/routstr/types";
import {
  connectToRoutstrService,
  disconnectFromRoutstrService,
  refreshRoutstrModels,
  getRoutstrModels,
  getRoutstrConnectionStatus,
  createRoutstrWallet,
  createBalanceWithToken,
  getRoutstrWalletBalance,
  topUpRoutstrWallet,
  refundRoutstrWallet,
  setRoutstrAutoTopupConfig,
  getRoutstrAutoTopupConfig,
  getStoredRoutstrApiKey,
  clearRoutstrConfig,
} from "@/lib/routstr/api";
import {
  discoverNostrProviders,
  type NostrProvider,
} from "@/lib/nostr-providers";

type RoutstrScreenProps = {
  copyToClipboard: (value: string) => Promise<void> | void;
};

const defaultAmount = 5000;

export function RoutstrScreen({ copyToClipboard }: RoutstrScreenProps) {
  const [serviceUrl, setServiceUrl] = useState("");
  const [connecting, setConnecting] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [connectionStatus, setConnectionStatus] =
    useState<RoutstrConnectionStatus>({
      connected: false,
      base_url: null,
      model_count: 0,
      has_api_key: false,
    });
  const [models, setModels] = useState<RoutstrModel[]>([]);
  const [selectedModel, setSelectedModel] = useState<string>("");
  const [error, setError] = useState<string | null>(null);
  const [successMessage, setSuccessMessage] = useState<string | null>(null);

  const [authAmount, setAuthAmount] = useState(defaultAmount);
  const [apiKey, setApiKey] = useState("");
  const [walletBalance, setWalletBalance] =
    useState<RoutstrWalletBalance | null>(null);
  const [topUpAmount, setTopUpAmount] = useState(defaultAmount);
  const [walletLoading, setWalletLoading] = useState(false);

  const [createServiceUrl, setCreateServiceUrl] = useState("");
  const [createAmount, setCreateAmount] = useState(defaultAmount);
  const [creating, setCreating] = useState(false);

  const [autoTopupConfig, setAutoTopupConfig] =
    useState<RoutstrAutoTopupConfig>({
      enabled: false,
      min_threshold: 20000,
      target_amount: 50000,
    });
  const [autoTopupLoading, setAutoTopupLoading] = useState(false);

  const [localWalletBalance, setLocalWalletBalance] = useState<number>(0);

  const [providers, setProviders] = useState<NostrProvider[]>([]);
  const [selectedProvider, setSelectedProvider] = useState<string>("");
  const [loadingProviders, setLoadingProviders] = useState(false);
  const [useManualUrl, setUseManualUrl] = useState(true);

  const createTokenFromLocalWallet = async (
    amount_sats: number,
  ): Promise<string> => {
    return invoke("create_external_token", {
      amountSats: amount_sats,
      mintUrl: null,
    });
  };

  const getLocalWalletBalance = async (): Promise<number> => {
    return invoke("get_wallet_balance");
  };

  const refreshLocalWalletBalance = async () => {
    try {
      const balance = await getLocalWalletBalance();
      setLocalWalletBalance(balance);
    } catch (error) {
      console.error("Failed to get local wallet balance:", error);
    }
  };

  const loadProviders = async () => {
    setLoadingProviders(true);
    try {
      const discoveredProviders = await discoverNostrProviders();
      const sortedProviders = discoveredProviders.sort((a, b) => {
        if (a.is_official && !b.is_official) return -1;
        if (!a.is_official && b.is_official) return 1;
        return a.name.localeCompare(b.name);
      });
      setProviders(sortedProviders);
      if (sortedProviders.length > 0 && !selectedProvider) {
        const defaultProvider =
          sortedProviders.find((p) => p.is_official) || sortedProviders[0];
        setSelectedProvider(defaultProvider.id);
      }
      setLoadingProviders(false);
    } catch (error) {
      console.error("Failed to discover providers:", error);
      setError(`Failed to discover providers: ${error}`);
    } finally {
      setLoadingProviders(false);
    }
  };

  const refreshConnectionStatus = async () => {
    try {
      const status = await getRoutstrConnectionStatus();
      setConnectionStatus(status);
      if (status.connected && status.base_url) {
        setServiceUrl(status.base_url);
        const modelsData = await getRoutstrModels();
        setModels(modelsData);
      }
    } catch (error) {
      console.error("Failed to refresh connection status:", error);
    }
  };

  const handleConnect = async () => {
    let urlToConnect = "";

    if (useManualUrl) {
      if (!serviceUrl.trim()) {
        setError("Please enter a service URL");
        return;
      }
      urlToConnect = serviceUrl.trim();
    } else {
      const provider = providers.find((p) => p.id === selectedProvider);
      if (!provider || !provider.urls.length) {
        setError("Please select a valid provider");
        return;
      }
      urlToConnect = provider.urls[0];
    }

    setConnecting(true);
    setError(null);
    setSuccessMessage(null);
    try {
      await connectToRoutstrService(urlToConnect);
      await refreshConnectionStatus();
    } catch (error) {
      console.error("Failed to connect to Routstr service:", error);
      setError(`Failed to connect: ${error}`);
    } finally {
      setConnecting(false);
    }
  };

  const handleDisconnect = async () => {
    try {
      await disconnectFromRoutstrService();
      setModels([]);
      setSelectedModel("");
      await refreshConnectionStatus();
    } catch (error) {
      console.error("Failed to disconnect:", error);
      setError(`Failed to disconnect: ${error}`);
    }
  };

  const handleCreateWallet = async () => {
    let urlToUse = "";

    if (useManualUrl) {
      if (!createServiceUrl.trim()) {
        setError("Please enter a service URL");
        return;
      }
      urlToUse = createServiceUrl.trim();
    } else {
      const provider = providers.find((p) => p.id === selectedProvider);
      if (!provider || !provider.urls.length) {
        setError("Please select a valid provider");
        return;
      }
      urlToUse = provider.urls[0];
    }

    setCreating(true);
    setError(null);
    setSuccessMessage(null);
    try {
      const cashu_token = await createTokenFromLocalWallet(createAmount);

      const result = await createRoutstrWallet(urlToUse, cashu_token);
      setCreateServiceUrl("");
      setApiKey(result.api_key);

      await refreshConnectionStatus();
      await refreshLocalWalletBalance();

      setSuccessMessage(
        `Wallet created successfully using ${createAmount.toLocaleString()} sats from local wallet! Balance: ${result.balance} msats`,
      );
    } catch (error) {
      console.error("Failed to create wallet:", error);
      let errorMsg = `Failed to create wallet: ${error}`;

      if (errorMsg.includes("Insufficient balance")) {
        errorMsg += "\n\nPlease add funds to your local wallet first.";
      }

      setError(errorMsg);
    } finally {
      setCreating(false);
    }
  };

  const handleRefreshModels = async () => {
    setRefreshing(true);
    setError(null);
    setSuccessMessage(null);
    try {
      await refreshRoutstrModels();
      const modelsData = await getRoutstrModels();
      setModels(modelsData);
      await refreshConnectionStatus();
    } catch (error) {
      console.error("Failed to refresh models:", error);
      setError(`Failed to refresh models: ${error}`);
    } finally {
      setRefreshing(false);
    }
  };

  const handleCreateAuth = async () => {
    setWalletLoading(true);
    setError(null);
    setSuccessMessage(null);
    try {
      const cashu_token = await createTokenFromLocalWallet(authAmount);

      const result = await createBalanceWithToken(cashu_token);
      setApiKey(result.api_key);

      await refreshConnectionStatus();
      await refreshWalletBalance();
      await refreshLocalWalletBalance();

      setSuccessMessage(
        `Balance created successfully using ${authAmount.toLocaleString()} sats from local wallet! Balance: ${result.balance} msats`,
      );
    } catch (error) {
      console.error("Failed to create balance:", error);
      let errorMsg = `Failed to create balance: ${error}`;

      if (errorMsg.includes("Insufficient balance")) {
        errorMsg += "\n\nPlease add funds to your local wallet first.";
      }

      setError(errorMsg);
    } finally {
      setWalletLoading(false);
    }
  };

  const refreshWalletBalance = async () => {
    if (!connectionStatus.has_api_key) return;

    try {
      const balance = await getRoutstrWalletBalance();
      setWalletBalance(balance);
    } catch (error) {
      console.error("Failed to get wallet balance:", error);
      setError(`Failed to get wallet balance: ${error}`);
    }
  };

  const handleTopUpWallet = async () => {
    setWalletLoading(true);
    setError(null);
    setSuccessMessage(null);
    try {
      const cashu_token = await createTokenFromLocalWallet(topUpAmount);

      await topUpRoutstrWallet(cashu_token);
      await refreshWalletBalance();
      await refreshLocalWalletBalance();

      setSuccessMessage(
        `Balance added successfully using ${topUpAmount.toLocaleString()} sats from local wallet!`,
      );
    } catch (error) {
      console.error("Failed to add balance:", error);
      let errorMsg = `Failed to add balance: ${error}`;

      if (errorMsg.includes("Insufficient balance")) {
        errorMsg += "\n\nPlease add funds to your local wallet first.";
      }

      setError(errorMsg);
    } finally {
      setWalletLoading(false);
    }
  };

  const loadAutoTopupConfig = async () => {
    try {
      const config = await getRoutstrAutoTopupConfig();
      setAutoTopupConfig(config);
    } catch (error) {
      console.error("Failed to load auto-topup config:", error);
    }
  };

  const loadStoredApiKey = async () => {
    try {
      const storedApiKey = await getStoredRoutstrApiKey();
      if (storedApiKey) {
        setApiKey(storedApiKey);
      } else {
        loadProviders();
      }
    } catch (error) {
      console.error("Failed to load stored API key:", error);
    }
  };

  const handleAutoTopupConfigChange = async () => {
    setAutoTopupLoading(true);
    setError(null);
    try {
      await setRoutstrAutoTopupConfig(
        autoTopupConfig.enabled,
        autoTopupConfig.min_threshold,
        autoTopupConfig.target_amount,
      );
    } catch (error) {
      console.error("Failed to update auto-topup config:", error);
      setError(`Failed to update auto-topup config: ${error}`);
    } finally {
      setAutoTopupLoading(false);
    }
  };

  const handleRecreateToken = async () => {
    if (!apiKey) {
      setError("No Cashu token available to recreate");
      return;
    }

    setWalletLoading(true);
    setError(null);
    setSuccessMessage(null);
    try {
      const result = await refundRoutstrWallet();

      await clearRoutstrConfig();
      setApiKey("");
      setWalletBalance(null);
      setAutoTopupConfig({
        enabled: false,
        min_threshold: 10000,
        target_amount: 50000,
      });

      try {
        await refreshConnectionStatus();
      } catch (statusError) {
        console.log(
          "Expected error during status refresh after token recreation:",
          statusError,
        );
      }

      await refreshLocalWalletBalance();

      if (result.token) {
        setSuccessMessage(
          "Token recreated successfully! Funds have been automatically added to your local wallet. Configuration has been reset.",
        );
      } else {
        setSuccessMessage(
          "Token recreation completed. Configuration has been reset.",
        );
      }
    } catch (error) {
      console.error("Failed to recreate token:", error);
      let errorMsg = `Failed to recreate token: ${error}`;

      if (
        errorMsg.includes("401") &&
        errorMsg.includes("Token already spent")
      ) {
        try {
          await clearRoutstrConfig();
          setApiKey("");
          setWalletBalance(null);
          setAutoTopupConfig({
            enabled: false,
            min_threshold: 10000,
            target_amount: 50000,
          });
          await refreshLocalWalletBalance();
          setSuccessMessage(
            "Token may have been refunded successfully, but configuration has been reset. Funds should be automatically added to your local wallet.",
          );
        } catch (clearError) {
          setError(
            `Token recreation failed and cleanup also failed: ${errorMsg}`,
          );
        }
      } else if (
        errorMsg.includes("401") &&
        errorMsg.includes("Invalid API key")
      ) {
        errorMsg +=
          "\n\nThe Cashu token may not be valid or may not have any balance to refund.";
        setError(errorMsg);
      } else {
        setError(errorMsg);
      }
    } finally {
      setWalletLoading(false);
    }
  };

  useEffect(() => {
    refreshConnectionStatus();
    loadStoredApiKey();
    refreshLocalWalletBalance();
  }, []);

  useEffect(() => {
    if (connectionStatus.connected && connectionStatus.has_api_key) {
      refreshWalletBalance();
      loadAutoTopupConfig();
    }
  }, [connectionStatus.connected, connectionStatus.has_api_key]);

  useEffect(() => {
    if (!connectionStatus.connected || !connectionStatus.has_api_key) {
      return;
    }

    const interval = setInterval(() => {
      refreshWalletBalance();
    }, 30000);

    return () => clearInterval(interval);
  }, [connectionStatus.connected, connectionStatus.has_api_key]);

  const selectedModelData = models.find((model) => model.id === selectedModel);

  return (
    <Screen className="min-h-screen gap-6 overflow-y-auto pt-6">
      <SectionHeader
        title="Routstr Settings"
        description="Connect to a Routstr"
      />

      <div className="space-y-6">
        <Card>
          <CardHeader>
            <CardTitle>Service Connection</CardTitle>
            <CardDescription>Connect to a Routstr Provider</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-4">
              <div className="flex items-center space-x-2">
                <input
                  type="radio"
                  id="manual-url"
                  checked={useManualUrl}
                  onChange={() => setUseManualUrl(true)}
                  className="w-4 h-4"
                  disabled={connecting || connectionStatus.connected}
                />
                <Label htmlFor="manual-url">Enter URL manually</Label>
              </div>

              {useManualUrl && (
                <div className="space-y-2 ml-6">
                  <Label htmlFor="service-url">Service URL</Label>
                  <Input
                    id="service-url"
                    placeholder="https://api.routstr.com"
                    value={serviceUrl}
                    onChange={(e) => setServiceUrl(e.target.value)}
                    disabled={connecting || connectionStatus.connected}
                  />
                </div>
              )}

              <div className="flex items-center space-x-2">
                <input
                  type="radio"
                  id="select-provider"
                  checked={!useManualUrl}
                  onChange={() => setUseManualUrl(false)}
                  className="w-4 h-4"
                  disabled={connecting || connectionStatus.connected}
                />
                <Label htmlFor="select-provider">
                  Select from discovered providers
                </Label>
                <Button
                  onClick={loadProviders}
                  disabled={
                    loadingProviders || connecting || connectionStatus.connected
                  }
                  variant="outline"
                  size="sm"
                >
                  {loadingProviders ? "Loading..." : "Refresh"}
                </Button>
              </div>

              {!useManualUrl && (
                <div className="space-y-2 ml-6">
                  <Label>Available Providers</Label>
                  {providers.length > 0 ? (
                    <Select
                      value={selectedProvider}
                      onValueChange={setSelectedProvider}
                    >
                      <SelectTrigger
                        className="w-full"
                        disabled={connecting || connectionStatus.connected}
                      >
                        <SelectValue placeholder="Choose a provider..." />
                      </SelectTrigger>
                      <SelectContent
                        className="max-h-[200px] w-[var(--radix-select-trigger-width)] overflow-y-auto"
                        position="popper"
                        side="bottom"
                        align="center"
                        sideOffset={0}
                        avoidCollisions={true}
                        collisionPadding={20}
                      >
                        {providers.map((provider) => (
                          <SelectItem key={provider.id} value={provider.id}>
                            <div className="flex flex-col justify-start">
                              <div className="flex items-center gap-2">
                                <span className="font-medium">
                                  {provider.name}
                                </span>
                                {provider.is_official && (
                                  <Badge tone="success" className="py-0">
                                    Official
                                  </Badge>
                                )}
                              </div>
                              <span className="text-xs text-muted-foreground self-start">
                                {provider.urls[0]}
                              </span>
                            </div>
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  ) : (
                    <p className="text-sm text-muted-foreground">
                      {loadingProviders
                        ? "Discovering providers..."
                        : "No providers found"}
                    </p>
                  )}
                </div>
              )}
            </div>

            {error && (
              <div className="text-sm text-red-500 whitespace-pre-line">
                {error}
              </div>
            )}

            {successMessage && (
              <div className="text-sm text-green-600">{successMessage}</div>
            )}

            <div className="flex items-center justify-between gap-2">
              {connectionStatus.connected ? (
                <>
                  <Badge tone="default">Connected</Badge>
                  <Button
                    onClick={handleDisconnect}
                    variant="outline"
                    size="sm"
                  >
                    Disconnect
                  </Button>
                </>
              ) : (
                <Button onClick={handleConnect} disabled={connecting} size="sm">
                  {connecting ? "Connecting..." : "Connect"}
                </Button>
              )}
            </div>
          </CardContent>
        </Card>

        {!connectionStatus.connected && (
          <Card>
            <CardHeader>
              <CardTitle>Create New Wallet</CardTitle>
              <CardDescription>
                Create a new wallet using a Cashu token. This will establish
                both connection and authentication in one step.
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="space-y-2">
                <Label htmlFor="create-service-url">Service URL</Label>
                <Input
                  id="create-service-url"
                  placeholder="https://api.routstr.com"
                  value={createServiceUrl}
                  onChange={(e) => setCreateServiceUrl(e.target.value)}
                  disabled={creating}
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="create-amount">Amount (sats)</Label>
                <Input
                  id="create-amount"
                  type="number"
                  placeholder="10000"
                  value={createAmount}
                  onChange={(e) => setCreateAmount(Number(e.target.value) || 0)}
                  disabled={creating}
                  min="1000"
                  step="1000"
                />
                <p className="text-xs text-muted-foreground">
                  Amount will be taken from your local wallet to create the
                  Routstr wallet.
                </p>
              </div>
              <div className="space-y-3">
                <Button
                  onClick={handleCreateWallet}
                  disabled={
                    creating ||
                    !createServiceUrl.trim() ||
                    localWalletBalance < createAmount ||
                    createAmount < 1000
                  }
                  className="w-full"
                >
                  {creating
                    ? "Creating Wallet..."
                    : `Create Wallet (${createAmount.toLocaleString()} sats)`}
                </Button>

                <div className="relative">
                  <div className="absolute inset-0 flex items-center">
                    <span className="w-full border-t" />
                  </div>
                  <div className="relative flex justify-center text-xs uppercase">
                    <span className="bg-background px-2 text-muted-foreground">
                      Or
                    </span>
                  </div>
                </div>

                <p className="text-xs text-center text-muted-foreground">
                  Local wallet balance: {localWalletBalance.toLocaleString()}{" "}
                  sats
                  {localWalletBalance < createAmount && (
                    <span className="text-red-500 block">
                      (Insufficient funds - need at least{" "}
                      {createAmount.toLocaleString()} sats)
                    </span>
                  )}
                </p>
              </div>
            </CardContent>
          </Card>
        )}

        {connectionStatus.connected && !connectionStatus.has_api_key && (
          <Card>
            <CardHeader>
              <CardTitle>Cashu Token Authentication</CardTitle>
              <CardDescription>
                Use your Cashu token to authenticate and create wallet balance
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="space-y-2">
                <Label htmlFor="auth-amount">Amount (sats)</Label>
                <Input
                  id="auth-amount"
                  type="number"
                  placeholder="10000"
                  value={authAmount}
                  onChange={(e) => setAuthAmount(Number(e.target.value) || 0)}
                  disabled={walletLoading}
                  min="1000"
                  step="1000"
                />
                <p className="text-xs text-muted-foreground">
                  Amount will be taken from your local wallet for
                  authentication.
                </p>
              </div>
              <div className="space-y-3">
                <Button
                  onClick={handleCreateAuth}
                  disabled={walletLoading || localWalletBalance < authAmount}
                  className="w-full"
                >
                  {walletLoading
                    ? "Creating..."
                    : `Create Authentication (${authAmount.toLocaleString()} sats)`}
                </Button>

                <div className="relative">
                  <div className="absolute inset-0 flex items-center">
                    <span className="w-full border-t" />
                  </div>
                  <div className="relative flex justify-center text-xs uppercase">
                    <span className="bg-background px-2 text-muted-foreground">
                      Or
                    </span>
                  </div>
                </div>

                <p className="text-xs text-center text-muted-foreground">
                  Local wallet balance: {localWalletBalance.toLocaleString()}{" "}
                  sats
                  {localWalletBalance < authAmount && (
                    <span className="text-red-500 block">
                      (Insufficient funds - need at least{" "}
                      {authAmount.toLocaleString()} sats)
                    </span>
                  )}
                </p>
              </div>
            </CardContent>
          </Card>
        )}

        {connectionStatus.connected && connectionStatus.has_api_key && (
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center justify-between">
                <span>Wallet Management</span>
                {walletBalance && (
                  <Badge tone="info">
                    {walletBalance.balance.toLocaleString()} msats
                  </Badge>
                )}
              </CardTitle>
              <CardDescription>
                Manage your Routstr wallet balance
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-6">
              {apiKey && (
                <div className="space-y-2">
                  <Label className="font-semibold">Current API Key</Label>
                  <div className="flex items-center gap-2 p-3 bg-muted rounded-lg">
                    <code className="flex-1 text-sm font-mono break-all">
                      {apiKey}
                    </code>
                    <CopyButton
                      label={""}
                      copiedLabel="Copied!"
                      onCopy={() => copyToClipboard(apiKey)}
                    />
                  </div>
                  <p className="text-xs text-muted-foreground">
                    This is your authentication token
                  </p>
                </div>
              )}

              {walletBalance && (
                <div className="p-4 rounded-lg bg-muted">
                  <div className="grid grid-cols-2 gap-4 text-sm">
                    <div>
                      <Label className="font-semibold">
                        Available Balance:
                      </Label>
                      <p>{walletBalance.balance.toLocaleString()} msats</p>
                    </div>
                  </div>
                </div>
              )}

              <div className="space-y-4 rounded-lg border p-4">
                <div>
                  <Label className="font-semibold">Top Up Wallet</Label>
                  <p className="text-sm text-muted-foreground">
                    Add funds using a new Cashu token. Your existing token will
                    be used for authentication.
                  </p>
                </div>
                <div className="space-y-2">
                  <Label htmlFor="topup-amount">Top-up Amount (sats)</Label>
                  <Input
                    id="topup-amount"
                    type="number"
                    placeholder="50000"
                    value={topUpAmount}
                    onChange={(e) =>
                      setTopUpAmount(Number(e.target.value) || 0)
                    }
                    disabled={walletLoading}
                    min="1000"
                    step="1000"
                  />
                  <p className="text-xs text-muted-foreground">
                    Amount will be taken from your local wallet to top up your
                    Routstr balance.
                  </p>
                </div>
                <Button
                  onClick={handleTopUpWallet}
                  disabled={
                    walletLoading ||
                    localWalletBalance < topUpAmount ||
                    topUpAmount < 1000
                  }
                  className="w-full"
                >
                  {walletLoading
                    ? "Processing..."
                    : `Top Up (${topUpAmount.toLocaleString()} sats)`}
                </Button>
                <p className="text-xs text-muted-foreground">
                  Local wallet balance: {localWalletBalance.toLocaleString()}{" "}
                  sats
                </p>
              </div>

              <div className="space-y-4 rounded-lg border border-red-200 bg-red-50 p-4">
                <div>
                  <Label className="font-semibold text-red-800">
                    Recreate Token
                  </Label>
                  <p className="text-sm text-red-700 mt-1">
                    Refund the balance and reset the configuration.
                  </p>
                </div>
                <Button
                  onClick={handleRecreateToken}
                  disabled={walletLoading}
                  size="sm"
                  variant="outline"
                  className="border-red-300 text-red-800 hover:bg-red-100"
                >
                  {walletLoading ? "Processing..." : "Recreate Token"}
                </Button>
              </div>
            </CardContent>
          </Card>
        )}

        {connectionStatus.connected && connectionStatus.has_api_key && (
          <Card>
            <CardHeader>
              <CardTitle>Auto-Topup Configuration</CardTitle>
              <CardDescription>
                Automatically monitor balance and alert when topup is needed
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="flex items-center space-x-2">
                <input
                  type="checkbox"
                  id="auto-topup-enabled"
                  checked={autoTopupConfig.enabled}
                  onChange={(e) =>
                    setAutoTopupConfig((prev) => ({
                      ...prev,
                      enabled: e.target.checked,
                    }))
                  }
                  disabled={autoTopupLoading}
                  className="h-4 w-4"
                />
                <Label htmlFor="auto-topup-enabled" className="font-semibold">
                  Enable Balance Monitoring
                </Label>
              </div>

              <div className="space-y-4">
                <div className="space-y-2">
                  <Label htmlFor="min-threshold">
                    Minimum Balance Threshold (msats)
                  </Label>
                  <Input
                    id="min-threshold"
                    type="number"
                    placeholder="10000"
                    value={autoTopupConfig.min_threshold}
                    onChange={(e) =>
                      setAutoTopupConfig((prev) => ({
                        ...prev,
                        min_threshold: parseInt(e.target.value) || 0,
                      }))
                    }
                    disabled={autoTopupLoading}
                  />
                  <p className="text-xs text-muted-foreground">
                    Alert when balance falls below this amount
                  </p>
                </div>

                <div className="space-y-2">
                  <Label htmlFor="target-amount">
                    Target Balance Amount (msats)
                  </Label>
                  <Input
                    id="target-amount"
                    type="number"
                    placeholder="100000"
                    value={autoTopupConfig.target_amount}
                    onChange={(e) =>
                      setAutoTopupConfig((prev) => ({
                        ...prev,
                        target_amount: parseInt(e.target.value) || 0,
                      }))
                    }
                    disabled={autoTopupLoading}
                  />
                  <p className="text-xs text-muted-foreground">
                    Desired balance to reach when topping up
                  </p>
                </div>

                <Button
                  onClick={handleAutoTopupConfigChange}
                  disabled={autoTopupLoading}
                  size="sm"
                >
                  {autoTopupLoading ? "Saving..." : "Save Configuration"}
                </Button>

                {autoTopupConfig.enabled && (
                  <div className="p-3 rounded-lg bg-blue-50 border border-blue-200">
                    <p className="text-sm text-blue-800">
                      <strong>Balance monitoring active:</strong> The system
                      will check your balance every 30 seconds and log alerts
                      when it falls below{" "}
                      {autoTopupConfig.min_threshold.toLocaleString()} msats.
                    </p>
                  </div>
                )}
              </div>
            </CardContent>
          </Card>
        )}

        {connectionStatus.connected && (
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center justify-between">
                <span>Available Models</span>
                <Button
                  onClick={handleRefreshModels}
                  disabled={refreshing}
                  variant="outline"
                  size="sm"
                >
                  {refreshing ? "Refreshing..." : "Refresh"}
                </Button>
              </CardTitle>
              <CardDescription>
                Select a model to view its details
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="space-y-2 relative">
                <Label>Select Model</Label>
                <div className="relative overflow-visible">
                  <Select
                    value={selectedModel}
                    onValueChange={setSelectedModel}
                  >
                    <SelectTrigger className="w-full max-w-full">
                      <SelectValue placeholder="Choose a model..." />
                    </SelectTrigger>
                    <SelectContent
                      className="max-h-[200px] w-[var(--radix-select-trigger-width)] overflow-y-auto"
                      position="popper"
                      side="bottom"
                      align="center"
                      sideOffset={0}
                      avoidCollisions={true}
                      collisionPadding={20}
                    >
                      {models
                        .slice()
                        .sort((a, b) => a.name.localeCompare(b.name))
                        .map((model) => (
                          <SelectItem key={model.id} value={model.id}>
                            {model.name}
                          </SelectItem>
                        ))}
                    </SelectContent>
                  </Select>
                </div>
              </div>

              {selectedModelData && (
                <div className="space-y-4 rounded-lg border p-4">
                  <div className="space-y-2">
                    <div className="flex items-center gap-2">
                      <Label className="font-semibold">Model ID:</Label>
                      <code className="text-sm bg-muted px-2 py-1 rounded">
                        {selectedModelData.id}
                      </code>
                      <CopyButton
                        copiedLabel="Copied!"
                        onCopy={() => copyToClipboard(selectedModelData.id)}
                        label={selectedModelData.id}
                      />
                    </div>
                    <div>
                      <Label className="font-semibold">Description:</Label>
                      <p className="text-sm text-muted-foreground mt-1">
                        {selectedModelData.description}
                      </p>
                    </div>
                    <div className="grid grid-cols-2 gap-4">
                      <div>
                        <Label className="font-semibold">Context Length:</Label>
                        <p className="text-sm">
                          {selectedModelData.context_length.toLocaleString()}{" "}
                          tokens
                        </p>
                      </div>
                      <div>
                        <Label className="font-semibold">Modality:</Label>
                        <p className="text-sm">
                          {selectedModelData.architecture.modality}
                        </p>
                      </div>
                    </div>
                    <div>
                      <Label className="font-semibold">Pricing (sats):</Label>
                      <div className="text-sm space-y-1 mt-1">
                        <div>
                          Prompt:{" "}
                          {selectedModelData.sats_pricing.prompt.toFixed(8)}{" "}
                          sats/token
                        </div>
                        <div>
                          Completion:{" "}
                          {selectedModelData.sats_pricing.completion.toFixed(8)}{" "}
                          sats/token
                        </div>
                        <div>
                          Request:{" "}
                          {selectedModelData.sats_pricing.request.toFixed(3)}{" "}
                          sats
                        </div>
                      </div>
                    </div>
                  </div>
                </div>
              )}
            </CardContent>
          </Card>
        )}
      </div>
    </Screen>
  );
}
