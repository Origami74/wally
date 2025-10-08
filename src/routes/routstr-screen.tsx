import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";

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
import {
  connectToRoutstrService,
  disconnectFromRoutstrService,
  refreshRoutstrModels,
  getRoutstrModels,
  getRoutstrConnectionStatus,
  createRoutstrWallet,
  createBalanceWithToken,
  clearRoutstrConfig,
  getAllApiKeys,
  getAllWalletBalances,
  refundWalletForKey,
  forceResetAllApiKeys,
  topUpWalletForKey,
  getProxyStatus,
  setUIState,
  getUIState,
} from "@/lib/routstr/api";
import { discoverNostrProviders } from "@/lib/nostr-providers";

type RoutstrScreenProps = {
  copyToClipboard: (value: string) => Promise<void> | void;
};

const defaultAmount = 5000;

export function RoutstrScreen({ copyToClipboard }: RoutstrScreenProps) {
  const [serviceUrl, setServiceUrl] = useState("");
  const [connecting, setConnecting] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [selectedModel, setSelectedModel] = useState<string>("");
  const [error, setError] = useState<string | null>(null);
  const [successMessage, setSuccessMessage] = useState<string | null>(null);

  const [authAmount, setAuthAmount] = useState(defaultAmount);
  const [topUpAmount, setTopUpAmount] = useState(defaultAmount);
  const [selectedTopUpKey, setSelectedTopUpKey] = useState<string>("");
  const [walletLoading, setWalletLoading] = useState(false);

  const [createServiceUrl, setCreateServiceUrl] = useState("");
  const [createAmount, setCreateAmount] = useState(defaultAmount);
  const [creating, setCreating] = useState(false);

  const [localWalletBalance, setLocalWalletBalance] = useState<number>(0);

  const [selectedProvider, setSelectedProvider] = useState<string>("");
  const [useManualUrl, setUseManualUrl] = useState(true);
  const [serviceMode, setServiceMode] = useState<"wallet" | "proxy">("wallet");
  const queryClient = useQueryClient();

  const proxyEndpoint = "http://127.0.0.1:3737";

  const {
    data: providers = [],
    isLoading: loadingProviders,
    refetch: refetchProviders,
  } = useQuery({
    queryKey: ["nostr-providers"],
    queryFn: async () => {
      const discoveredProviders = await discoverNostrProviders();
      const sortedProviders = discoveredProviders.sort((a, b) => {
        if (a.is_official && !b.is_official) return -1;
        if (!a.is_official && b.is_official) return 1;
        return a.name.localeCompare(b.name);
      });
      return sortedProviders;
    },
    staleTime: 5 * 60 * 1000,
    enabled: !useManualUrl,
  });

  const {
    data: connectionStatus = {
      connected: false,
      base_url: null,
      model_count: 0,
      has_api_key: false,
    },
    refetch: refetchConnectionStatus,
  } = useQuery({
    queryKey: ["routstr-connection-status"],
    queryFn: getRoutstrConnectionStatus,
    refetchInterval: 30000,
    staleTime: 10000,
  });

  const {
    data: proxyStatus = {
      use_proxy: false,
      proxy_endpoint: null,
      target_service_url: null,
    },
    refetch: refetchProxyStatus,
  } = useQuery({
    queryKey: ["routstr-proxy-status"],
    queryFn: getProxyStatus,
    refetchInterval: 30000,
    staleTime: 10000,
  });

  const { data: models = [], refetch: refetchModels } = useQuery({
    queryKey: ["routstr-models"],
    queryFn: getRoutstrModels,
    enabled: connectionStatus.connected,
    staleTime: 10 * 60 * 1000,
  });

  const { data: apiKeys = [], refetch: refetchApiKeys } = useQuery({
    queryKey: ["routstr-api-keys"],
    queryFn: getAllApiKeys,
    enabled: connectionStatus.connected,
    staleTime: 10000,
  });

  const { data: walletBalances = [], refetch: refetchWalletBalances } =
    useQuery({
      queryKey: ["routstr-wallet-balances"],
      queryFn: getAllWalletBalances,
      enabled: connectionStatus.connected && apiKeys.length > 0,
      refetchInterval: 30000,
      staleTime: 10000,
    });

  useEffect(() => {
    if (providers.length > 0 && !selectedProvider) {
      const defaultProvider =
        providers.find((p) => p.is_official) || providers[0];
      setSelectedProvider(defaultProvider.id);
    }
  }, [providers, selectedProvider]);

  useEffect(() => {
    if (apiKeys.length > 0 && !selectedTopUpKey) {
      setSelectedTopUpKey(apiKeys[0].api_key);
    }
  }, [apiKeys, selectedTopUpKey]);

  useEffect(() => {
    if (connectionStatus.base_url && connectionStatus.connected) {
      setServiceUrl(connectionStatus.base_url);
    } else if (!connectionStatus.connected) {
      setServiceUrl("");
    }
  }, [connectionStatus.base_url, connectionStatus.connected]);

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

  const handleConnect = async () => {
    let urlToConnect = "";

    // Determine the service URL to connect to
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
    console.log(urlToConnect, useManualUrl, serviceMode);
    connectMutation.mutate({
      url: urlToConnect,
      useManualUrl,
      selectedProviderId: selectedProvider,
      serviceMode,
    });
  };

  const handleDisconnect = async () => {
    try {
      await disconnectFromRoutstrService();

      if (proxyStatus.use_proxy) {
        await refetchProxyStatus();
      }

      setSelectedModel("");
      await refetchConnectionStatus();
      queryClient.invalidateQueries({ queryKey: ["routstr-models"] });
      queryClient.invalidateQueries({ queryKey: ["routstr-api-keys"] });
      queryClient.invalidateQueries({ queryKey: ["routstr-wallet-balances"] });
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
      createWalletMutation.mutate({ url: urlToUse, token: cashu_token });
    } catch (error) {
      console.error("Failed to create wallet token:", error);
      setError(`Failed to create wallet token: ${error}`);
      setCreating(false);
    }
  };

  const handleCreateAuth = async () => {
    setWalletLoading(true);
    setError(null);
    setSuccessMessage(null);
    try {
      const cashu_token = await createTokenFromLocalWallet(authAmount);

      const result = await createBalanceWithToken(cashu_token);

      await refetchConnectionStatus();
      await refetchApiKeys();
      await refetchWalletBalances();
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

  const handleTopUpWallet = async () => {
    if (!selectedTopUpKey) {
      setError("Please select an API key to top up");
      return;
    }

    setWalletLoading(true);
    setError(null);
    setSuccessMessage(null);
    try {
      const cashu_token = await createTokenFromLocalWallet(topUpAmount);
      topUpMutation.mutate({
        apiKey: selectedTopUpKey,
        cashuToken: cashu_token,
      });
    } catch (error) {
      console.error("Failed to create topup token:", error);
      setError(`Failed to create topup token: ${error}`);
      setWalletLoading(false);
    }
  };

  const loadStoredApiKey = async () => {
    try {
      const apiKeys = await getAllApiKeys();
      if (apiKeys.length > 0) {
      } else {
        if (!useManualUrl) {
          refetchProviders();
        }
      }
    } catch (error) {
      console.error("Failed to load stored API keys:", error);
    }
  };

  const handleRefundSpecificWallet = async (apiKey: string) => {
    setWalletLoading(true);
    setError(null);
    setSuccessMessage(null);
    try {
      const result = await refundWalletForKey(apiKey);

      await refetchWalletBalances();
      await refreshLocalWalletBalance();

      if (result.token) {
        setSuccessMessage(
          `Wallet refunded successfully for API key ending in ...${apiKey.slice(-8)}! Funds have been added to your local wallet.`,
        );
      } else {
        setSuccessMessage(
          `Refund completed for API key ending in ...${apiKey.slice(-8)}.`,
        );
      }
    } catch (error) {
      console.error("Failed to refund specific wallet:", error);
      setError(`Failed to refund wallet: ${error}`);
    } finally {
      setWalletLoading(false);
    }
  };

  const handleForceResetAllApiKeys = async () => {
    setWalletLoading(true);
    setError(null);
    setSuccessMessage(null);

    const refundResults = [];
    for (const apiKeyEntry of apiKeys) {
      try {
        const result = await refundWalletForKey(apiKeyEntry.api_key);
        refundResults.push(result);
      } catch (error) {
        console.warn(
          `Failed to refund API key ${apiKeyEntry.api_key}, continuing with force reset:`,
          error,
        );
      }
    }

    try {
      await forceResetAllApiKeys();
      await clearRoutstrConfig();

      await refetchConnectionStatus();
      queryClient.invalidateQueries({ queryKey: ["routstr-api-keys"] });
      queryClient.invalidateQueries({ queryKey: ["routstr-wallet-balances"] });

      await refreshLocalWalletBalance();
    } catch (error) {
      console.warn("Failed to reset config", error);
    }

    const successfulRefunds = refundResults.filter((r) => r.token).length;
    const totalKeys = apiKeys.length;

    setSuccessMessage(
      `Force reset completed! All ${totalKeys} API keys have been deleted. ${successfulRefunds} wallets were successfully refunded and funds added to your local wallet.`,
    );
    setWalletLoading(false);
  };

  useEffect(() => {
    loadStoredApiKey();
    refreshLocalWalletBalance();
    loadUIState();
  }, []);

  const loadUIState = async () => {
    try {
      const uiState = await getUIState();
      console.log(uiState);
      setUseManualUrl(uiState.use_manual_url);
      if (uiState.selected_provider_id) {
        setSelectedProvider(uiState.selected_provider_id);
      }
      setServiceMode(uiState.service_mode as "wallet" | "proxy");
    } catch (error) {
      console.error("Failed to load UI state:", error);
    }
  };

  const saveUIState = async () => {
    try {
      await setUIState(useManualUrl, selectedProvider || null, serviceMode);
    } catch (error) {
      console.error("Failed to save UI state:", error);
    }
  };

  useEffect(() => {
    saveUIState();
  }, [useManualUrl, selectedProvider, serviceMode]);

  useEffect(() => {
    if (proxyStatus) {
      if (proxyStatus.target_service_url) {
        setServiceUrl(proxyStatus.target_service_url);
      }

      setUseManualUrl(!proxyStatus.use_proxy);
      setServiceMode(proxyStatus.use_proxy ? "wallet" : "proxy");
    }
  }, [proxyStatus, connectionStatus.connected]);

  const connectMutation = useMutation({
    mutationFn: (params: {
      url: string;
      useManualUrl: boolean;
      selectedProviderId: string | null;
      serviceMode: string;
    }) =>
      connectToRoutstrService(
        params.url,
        params.useManualUrl,
        params.selectedProviderId || undefined,
        params.serviceMode,
      ),
    onSuccess: () => {
      refetchConnectionStatus();
      queryClient.invalidateQueries({ queryKey: ["routstr-models"] });
      saveUIState();
    },
    onError: (error) => {
      console.error("Failed to connect to Routstr service:", error);
      setError(`Failed to connect: ${error}`);
    },
    onSettled: () => {
      setConnecting(false);
    },
  });

  const createWalletMutation = useMutation({
    mutationFn: async ({ url, token }: { url: string; token: string }) => {
      return createRoutstrWallet(url, token);
    },
    onSuccess: (result) => {
      setCreateServiceUrl("");
      refetchConnectionStatus();
      refreshLocalWalletBalance();
      queryClient.invalidateQueries({ queryKey: ["routstr-api-keys"] });
      queryClient.invalidateQueries({ queryKey: ["routstr-wallet-balances"] });
      setSuccessMessage(
        `Wallet created successfully using ${createAmount.toLocaleString()} sats from local wallet! Balance: ${result.balance} msats`,
      );
    },
    onError: (error) => {
      console.error("Failed to create wallet:", error);
      let errorMsg = `Failed to create wallet: ${error}`;
      if (errorMsg.includes("Insufficient balance")) {
        errorMsg += "\n\nPlease add funds to your local wallet first.";
      }
      setError(errorMsg);
    },
    onSettled: () => {
      setCreating(false);
    },
  });

  const topUpMutation = useMutation({
    mutationFn: async ({
      apiKey,
      cashuToken,
    }: {
      apiKey: string;
      cashuToken: string;
    }) => {
      return topUpWalletForKey(apiKey, cashuToken);
    },
    onSuccess: (result) => {
      refetchWalletBalances();
      refreshLocalWalletBalance();
      setSuccessMessage(
        `Wallet topped up successfully with ${topUpAmount.toLocaleString()} sats! Added: ${result.msats} msats`,
      );
    },
    onError: (error) => {
      console.error("Failed to top up wallet:", error);
      let errorMsg = `Failed to top up wallet: ${error}`;
      if (errorMsg.includes("Insufficient balance")) {
        errorMsg += "\n\nPlease add funds to your local wallet first.";
      }
      setError(errorMsg);
    },
    onSettled: () => {
      setWalletLoading(false);
    },
  });

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
            <div className="space-y-6">
              <div className="space-y-3">
                <h4 className="text-sm font-medium">Connection Method</h4>
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
                      onClick={() => refetchProviders()}
                      disabled={
                        loadingProviders ||
                        connecting ||
                        connectionStatus.connected
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
              </div>

              <div className="space-y-3">
                <h4 className="text-sm font-medium">Service Mode</h4>
                <div className="space-y-4">
                  <div className="flex items-center space-x-2">
                    <input
                      type="radio"
                      id="wallet-mode"
                      checked={serviceMode === "wallet"}
                      onChange={() => setServiceMode("wallet")}
                      className="w-4 h-4"
                      disabled={connecting || connectionStatus.connected}
                    />
                    <Label htmlFor="wallet-mode">Wallet/Balance Mode</Label>
                  </div>

                  <div className="flex items-center space-x-2">
                    <input
                      type="radio"
                      id="proxy-mode"
                      checked={serviceMode === "proxy"}
                      onChange={() => setServiceMode("proxy")}
                      className="w-4 h-4"
                      disabled={connecting || connectionStatus.connected}
                    />
                    <Label htmlFor="proxy-mode">Proxy Mode (OTRTA)</Label>
                  </div>

                  {serviceMode === "proxy" && (
                    <div className="p-3 rounded-lg bg-blue-50 border border-blue-200">
                      <p className="text-sm text-blue-700">
                        Proxy mode will connect to {proxyEndpoint} and forward
                        requests to{" "}
                        {serviceUrl || "your configured target service"}.
                      </p>
                    </div>
                  )}
                </div>
              </div>
            </div>

            {error && (
              <div className="text-sm text-red-500 whitespace-pre-line">
                {error}
              </div>
            )}

            {successMessage && (
              <div className="text-sm text-green-600">{successMessage}</div>
            )}

            <div className="flex flex-col gap-2">
              {connectionStatus.connected ? (
                <>
                  {proxyStatus.use_proxy && (
                    <Badge tone="default" className="ml-2">
                      Proxy: {proxyStatus.target_service_url}
                    </Badge>
                  )}
                  <div className="flex justify-between">
                    <Badge tone="default">Connected</Badge>
                    <Button
                      onClick={handleDisconnect}
                      variant="outline"
                      size="sm"
                    >
                      Disconnect
                    </Button>
                  </div>
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

        {connectionStatus.connected && serviceMode !== "proxy" && (
          <Card>
            <CardHeader>
              <CardTitle>Add New API Key</CardTitle>
              <CardDescription>
                Create a new API key and wallet balance using your Cashu tokens
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
                  Amount will be taken from your local wallet to create a new
                  API key and initial balance.
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
                    : `Create New API Key (${authAmount.toLocaleString()} sats)`}
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
                {walletBalances.length > 0 && (
                  <Badge tone="info">
                    {walletBalances
                      .reduce((total, wb) => total + wb.balance, 0)
                      .toLocaleString()}{" "}
                    msats total
                  </Badge>
                )}
              </CardTitle>
              <CardDescription>
                Manage your Routstr wallet balance
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-6">
              {apiKeys.length > 0 && (
                <div className="space-y-4">
                  <Label className="font-semibold">API Keys & Balances</Label>
                  {apiKeys.map((apiKeyEntry, index) => {
                    const balance = walletBalances.find(
                      (wb) => wb.api_key === apiKeyEntry.api_key,
                    );
                    return (
                      <div
                        key={apiKeyEntry.api_key}
                        className="p-4 rounded-lg bg-muted"
                      >
                        <div className="space-y-3">
                          <div className="flex items-center justify-between">
                            <div className="flex items-center gap-2">
                              <Label className="font-semibold">
                                API Key {index + 1}:
                              </Label>
                              {apiKeyEntry.alias && (
                                <Badge tone="default">
                                  {apiKeyEntry.alias}
                                </Badge>
                              )}
                            </div>
                            <div className="text-right">
                              <Label className="font-semibold">Balance:</Label>
                              <p className="text-sm">
                                {balance
                                  ? balance.balance.toLocaleString()
                                  : "Loading..."}{" "}
                                msats
                              </p>
                            </div>
                          </div>
                          <div className="flex items-center gap-2">
                            <code className="flex-1 text-xs font-mono break-all bg-background px-2 py-1 rounded">
                              {apiKeyEntry.api_key}
                            </code>
                            <CopyButton
                              label=""
                              copiedLabel="Copied!"
                              onCopy={() =>
                                copyToClipboard(apiKeyEntry.api_key)
                              }
                            />
                          </div>
                          <div className="text-xs text-muted-foreground">
                            Created:{" "}
                            {new Date(
                              apiKeyEntry.created_at * 1000,
                            ).toLocaleDateString()}
                            {apiKeyEntry.creation_cashu_token && (
                              <span className="ml-2">
                                • Created with Cashu token
                              </span>
                            )}
                          </div>
                          <div className="flex gap-2 pt-2">
                            <Button
                              onClick={() =>
                                handleRefundSpecificWallet(apiKeyEntry.api_key)
                              }
                              disabled={walletLoading}
                              size="sm"
                              variant="outline"
                              className="border-red-300 text-red-800 hover:bg-red-100"
                            >
                              Refund
                            </Button>
                          </div>
                        </div>
                      </div>
                    );
                  })}
                </div>
              )}

              <div className="space-y-4 rounded-lg border p-4">
                <div>
                  <Label className="font-semibold">Top Up Wallet</Label>
                  <p className="text-sm text-muted-foreground">
                    Add funds using a new Cashu token. Select which API key to
                    top up.
                  </p>
                </div>
                <div className="space-y-2">
                  <Label htmlFor="topup-key-select">
                    Select API Key to Top Up
                  </Label>
                  <Select
                    value={selectedTopUpKey}
                    onValueChange={setSelectedTopUpKey}
                  >
                    <SelectTrigger className="w-full" disabled={walletLoading}>
                      <SelectValue placeholder="Choose an API key..." />
                    </SelectTrigger>
                    <SelectContent>
                      {apiKeys.map((apiKeyEntry, index) => {
                        const balance = walletBalances.find(
                          (wb) => wb.api_key === apiKeyEntry.api_key,
                        );
                        return (
                          <SelectItem
                            key={apiKeyEntry.api_key}
                            value={apiKeyEntry.api_key}
                          >
                            <div className="flex flex-col">
                              <div className="flex items-center gap-2">
                                <span>API Key {index + 1}</span>
                                {apiKeyEntry.alias && (
                                  <Badge tone="default" className="text-xs">
                                    {apiKeyEntry.alias}
                                  </Badge>
                                )}
                              </div>
                              <span className="text-xs text-muted-foreground">
                                Balance:{" "}
                                {balance
                                  ? balance.balance.toLocaleString()
                                  : "Loading..."}{" "}
                                msats
                              </span>
                            </div>
                          </SelectItem>
                        );
                      })}
                    </SelectContent>
                  </Select>
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
                    !selectedTopUpKey ||
                    apiKeys.length === 0
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
                    Reset Configuration
                  </Label>
                  <p className="text-sm text-red-700 mt-1">
                    Refund the balance and reset the configuration.
                  </p>
                </div>
                <div className="flex gap-2">
                  <Button
                    onClick={handleForceResetAllApiKeys}
                    disabled={walletLoading || apiKeys.length === 0}
                    size="sm"
                    variant="outline"
                    className="border-red-500 text-red-900 hover:bg-red-200 bg-red-100"
                  >
                    {walletLoading ? "Processing..." : "Reset All"}
                  </Button>
                </div>
                <div className="text-xs text-red-600">
                  <p>
                    <strong>Force Reset All:</strong> Refund and deletes all API
                    keys locally even if remote refund fails.
                  </p>
                </div>
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
                  onClick={() => {
                    setRefreshing(true);
                    Promise.all([
                      refreshRoutstrModels(),
                      refetchModels(),
                      refetchConnectionStatus(),
                    ]).finally(() => setRefreshing(false));
                  }}
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
                    <div className="flex flex-col items-center gap-2">
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
                          {selectedModelData.context_length?.toLocaleString()}{" "}
                          tokens
                        </p>
                      </div>
                      <div>
                        <Label className="font-semibold">Modality:</Label>
                        <p className="text-sm">
                          {selectedModelData.architecture?.modality}
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
                          {selectedModelData.sats_pricing.max_cost.toFixed(3)}{" "}
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
