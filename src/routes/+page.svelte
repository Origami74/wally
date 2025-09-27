<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { registerListener } from "$lib/tollgate/network/pluginCommands";

  // State
  let autoTollgateEnabled = $state(false);
  let connectionStatus = $state("disconnected");
  let remainingTime = $state("--:--");
  let remainingData = $state("--");
  let usagePercentage = $state(0);
  let walletBalance = $state(0);
  let currentSession = $state(null);
  let isLoading = $state(false);

  // Cleanup functions
  let unsubscribers: (() => void)[] = [];

  onMount(async () => {
    // Load initial status
    await refreshStatus();

    // Set up network event listeners
    await registerListener("network-connected", handleNetworkConnected);
    await registerListener("network-disconnected", handleNetworkDisconnected);

    // Refresh status every 5 seconds
    const statusInterval = setInterval(refreshStatus, 5000);
    unsubscribers.push(() => clearInterval(statusInterval));
  });

  onDestroy(() => {
    unsubscribers.forEach(unsub => unsub());
  });

  async function refreshStatus() {
    try {
      const status = await invoke("get_tollgate_status");
      autoTollgateEnabled = status.auto_tollgate_enabled;
      walletBalance = status.wallet_balance;
      
      // Update connection status
      if (status.active_sessions && status.active_sessions.length > 0) {
        const session = status.active_sessions[0];
        currentSession = session;
        connectionStatus = session.status.toLowerCase();
        usagePercentage = session.usage_percentage * 100;
        
        // Update remaining time/data
        if (session.remaining_time_seconds !== null) {
          const minutes = Math.floor(session.remaining_time_seconds / 60);
          const seconds = session.remaining_time_seconds % 60;
          remainingTime = `${minutes}:${seconds.toString().padStart(2, '0')}`;
        } else if (session.remaining_data_bytes !== null) {
          remainingData = formatBytes(session.remaining_data_bytes);
        }
      } else {
        connectionStatus = status.current_network?.is_tollgate ? "available" : "disconnected";
        currentSession = null;
        usagePercentage = 0;
        remainingTime = "--:--";
        remainingData = "--";
      }
    } catch (error) {
      console.error("Failed to refresh status:", error);
    }
  }

  async function toggleAutoTollgate() {
    if (isLoading) return;
    
    isLoading = true;
    try {
      await invoke("toggle_auto_tollgate", { enabled: !autoTollgateEnabled });
      await refreshStatus();
    } catch (error) {
      console.error("Failed to toggle auto-tollgate:", error);
    } finally {
      isLoading = false;
    }
  }

  async function handleNetworkConnected(data: any) {
    console.log("Network connected event:", data);
    // The Rust backend will handle the network connection automatically
    await refreshStatus();
  }

  async function handleNetworkDisconnected() {
    console.log("Network disconnected event");
    await invoke("handle_network_disconnected");
    await refreshStatus();
  }

  function formatBytes(bytes: number): string {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + " " + sizes[i];
  }

  function getStatusColor(status: string): string {
    switch (status) {
      case "active": return "#10b981"; // green
      case "renewing": return "#f59e0b"; // amber
      case "available": return "#3b82f6"; // blue
      case "expired": return "#ef4444"; // red
      case "error": return "#ef4444"; // red
      default: return "#6b7280"; // gray
    }
  }

  function getStatusText(status: string): string {
    switch (status) {
      case "active": return "CONNECTED";
      case "renewing": return "RENEWING";
      case "available": return "AVAILABLE";
      case "expired": return "EXPIRED";
      case "error": return "ERROR";
      default: return "DISCONNECTED";
    }
  }
</script>

<main class="app">
  <div class="header">
    <h1>TollGate</h1>
    <div class="wallet-balance">
      {walletBalance} sats
    </div>
  </div>

  <div class="status-section">
    <div 
      class="status-indicator"
      style="background-color: {getStatusColor(connectionStatus)}"
    >
      <div class="status-text">{getStatusText(connectionStatus)}</div>
    </div>

    {#if currentSession}
      <div class="session-info">
        <div class="usage-bar">
          <div 
            class="usage-fill"
            style="width: {usagePercentage}%"
          ></div>
        </div>
        
        <div class="remaining-info">
          {#if remainingTime !== "--:--"}
            <div class="remaining-time">
              <span class="label">Time:</span>
              <span class="value">{remainingTime}</span>
            </div>
          {/if}
          
          {#if remainingData !== "--"}
            <div class="remaining-data">
              <span class="label">Data:</span>
              <span class="value">{remainingData}</span>
            </div>
          {/if}
        </div>
      </div>
    {/if}
  </div>

  <div class="toggle-section">
    <button 
      class="toggle-button {autoTollgateEnabled ? 'enabled' : 'disabled'}"
      class:loading={isLoading}
      onclick={toggleAutoTollgate}
      disabled={isLoading}
    >
      <div class="toggle-inner">
        {#if isLoading}
          <div class="spinner"></div>
        {:else}
          <span class="toggle-text">
            {autoTollgateEnabled ? 'ON' : 'OFF'}
          </span>
        {/if}
      </div>
    </button>
    
    <div class="toggle-description">
      {autoTollgateEnabled 
        ? "Auto-purchase is enabled. TollGate sessions will start automatically." 
        : "Auto-purchase is disabled. Enable to automatically connect to TollGates."}
    </div>
  </div>
</main>

<style>
  .app {
    min-height: 100vh;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 2rem;
    background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
    color: white;
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
  }

  .header {
    text-align: center;
    margin-bottom: 3rem;
  }

  .header h1 {
    font-size: 3rem;
    font-weight: 700;
    margin: 0 0 1rem 0;
    text-shadow: 0 2px 4px rgba(0, 0, 0, 0.3);
  }

  .wallet-balance {
    font-size: 1.2rem;
    opacity: 0.9;
    background: rgba(255, 255, 255, 0.1);
    padding: 0.5rem 1rem;
    border-radius: 20px;
    backdrop-filter: blur(10px);
  }

  .status-section {
    margin-bottom: 3rem;
    text-align: center;
  }

  .status-indicator {
    width: 200px;
    height: 200px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    margin: 0 auto 2rem auto;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
    transition: all 0.3s ease;
  }

  .status-text {
    font-size: 1.5rem;
    font-weight: 600;
    text-shadow: 0 2px 4px rgba(0, 0, 0, 0.3);
  }

  .session-info {
    max-width: 300px;
    margin: 0 auto;
  }

  .usage-bar {
    width: 100%;
    height: 8px;
    background: rgba(255, 255, 255, 0.2);
    border-radius: 4px;
    overflow: hidden;
    margin-bottom: 1rem;
  }

  .usage-fill {
    height: 100%;
    background: linear-gradient(90deg, #10b981, #f59e0b, #ef4444);
    transition: width 0.3s ease;
  }

  .remaining-info {
    display: flex;
    justify-content: space-between;
    gap: 1rem;
  }

  .remaining-time,
  .remaining-data {
    display: flex;
    flex-direction: column;
    align-items: center;
  }

  .label {
    font-size: 0.9rem;
    opacity: 0.8;
    margin-bottom: 0.25rem;
  }

  .value {
    font-size: 1.2rem;
    font-weight: 600;
  }

  .toggle-section {
    text-align: center;
  }

  .toggle-button {
    width: 200px;
    height: 200px;
    border-radius: 50%;
    border: none;
    cursor: pointer;
    transition: all 0.3s ease;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
    margin-bottom: 2rem;
    position: relative;
    overflow: hidden;
  }

  .toggle-button.enabled {
    background: linear-gradient(135deg, #10b981, #059669);
  }

  .toggle-button.disabled {
    background: linear-gradient(135deg, #6b7280, #4b5563);
  }

  .toggle-button:hover:not(:disabled) {
    transform: scale(1.05);
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.4);
  }

  .toggle-button:active:not(:disabled) {
    transform: scale(0.95);
  }

  .toggle-button:disabled {
    cursor: not-allowed;
    opacity: 0.7;
  }

  .toggle-inner {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    height: 100%;
  }

  .toggle-text {
    font-size: 2rem;
    font-weight: 700;
    color: white;
    text-shadow: 0 2px 4px rgba(0, 0, 0, 0.3);
  }

  .spinner {
    width: 40px;
    height: 40px;
    border: 4px solid rgba(255, 255, 255, 0.3);
    border-top: 4px solid white;
    border-radius: 50%;
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    0% { transform: rotate(0deg); }
    100% { transform: rotate(360deg); }
  }

  .toggle-description {
    max-width: 400px;
    font-size: 1rem;
    opacity: 0.9;
    line-height: 1.5;
    background: rgba(255, 255, 255, 0.1);
    padding: 1rem;
    border-radius: 12px;
    backdrop-filter: blur(10px);
  }

  /* Responsive design */
  @media (max-width: 768px) {
    .app {
      padding: 1rem;
    }

    .header h1 {
      font-size: 2.5rem;
    }

    .status-indicator,
    .toggle-button {
      width: 150px;
      height: 150px;
    }

    .toggle-text {
      font-size: 1.5rem;
    }

    .status-text {
      font-size: 1.2rem;
    }
  }
</style>
