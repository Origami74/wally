<script>
	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	
	let deviceInfo = {
		macAddress: 'Loading...',
		currentNetwork: 'Loading...',
		gatewayIp: 'Loading...',
		isTollgate: false,
		tollgateInfo: null,
		deviceId: 'Loading...',
		walletBalance: 'Loading...',
		activeSessions: [],
		networkQuality: 'Loading...'
	};
	
	let loading = true;
	let error = null;
	
	onMount(async () => {
		await loadDeviceInfo();
	});
	
	async function loadDeviceInfo() {
		try {
			loading = true;
			error = null;
			
			// Get MAC address
			try {
				const macResult = await invoke('get_mac_address');
				deviceInfo.macAddress = macResult.mac_address || 'Unknown';
			} catch (e) {
				deviceInfo.macAddress = `Error: ${e}`;
			}
			
			// Get current WiFi details
			try {
				const wifiResult = await invoke('get_current_wifi_details');
				if (wifiResult.wifi) {
					deviceInfo.currentNetwork = wifiResult.wifi.ssid || 'Not connected';
				} else {
					deviceInfo.currentNetwork = 'Not connected';
				}
			} catch (e) {
				deviceInfo.currentNetwork = `Error: ${e}`;
			}
			
			// Get gateway IP separately
			try {
				const gatewayResult = await invoke('get_gateway_ip');
				deviceInfo.gatewayIp = gatewayResult.gateway_ip || 'Unknown';
			} catch (e) {
				deviceInfo.gatewayIp = 'Unknown';
			}
			
			// Check if current network is a TollGate
			try {
				const tollgateResult = await invoke('detect_tollgate', { 
					gatewayIp: deviceInfo.gatewayIp,
					macAddress: deviceInfo.macAddress 
				});
				deviceInfo.isTollgate = tollgateResult.is_tollgate || false;
				deviceInfo.tollgateInfo = tollgateResult.advertisement || null;
			} catch (e) {
				deviceInfo.isTollgate = false;
				deviceInfo.tollgateInfo = null;
			}
			
			// Set device identifier based on MAC address
			deviceInfo.deviceId = `mac=${deviceInfo.macAddress}`;
			
			// Get wallet balance
			try {
				const balanceResult = await invoke('get_wallet_balance');
				deviceInfo.walletBalance = `${balanceResult} sats`;
			} catch (e) {
				deviceInfo.walletBalance = `Error: ${e}`;
			}
			
			// Get active sessions
			try {
				const sessionsResult = await invoke('get_active_sessions');
				deviceInfo.activeSessions = sessionsResult || [];
			} catch (e) {
				deviceInfo.activeSessions = [];
			}
			
		} catch (e) {
			error = `Failed to load device info: ${e}`;
		} finally {
			loading = false;
		}
	}
	
	function formatBytes(bytes) {
		if (bytes === 0) return '0 B';
		const k = 1024;
		const sizes = ['B', 'KB', 'MB', 'GB'];
		const i = Math.floor(Math.log(bytes) / Math.log(k));
		return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
	}
	
	function formatDuration(seconds) {
		const hours = Math.floor(seconds / 3600);
		const minutes = Math.floor((seconds % 3600) / 60);
		const secs = seconds % 60;
		
		if (hours > 0) {
			return `${hours}h ${minutes}m ${secs}s`;
		} else if (minutes > 0) {
			return `${minutes}m ${secs}s`;
		} else {
			return `${secs}s`;
		}
	}
</script>

<div class="developer-page">
	<h1>🔧 Developer Info</h1>
	
	{#if loading}
		<div class="loading">
			<div class="spinner"></div>
			<p>Loading device information...</p>
		</div>
	{:else if error}
		<div class="error">
			<p>❌ {error}</p>
			<button on:click={loadDeviceInfo} class="retry-btn">Retry</button>
		</div>
	{:else}
		<div class="info-sections">
			<!-- Device Information -->
			<section class="info-section">
				<h2>📱 Device Information</h2>
				<div class="info-grid">
					<div class="info-item">
						<span class="label">MAC Address:</span>
						<span class="value">{deviceInfo.macAddress}</span>
					</div>
					<div class="info-item">
						<span class="label">Device ID:</span>
						<span class="value">{deviceInfo.deviceId}</span>
					</div>
				</div>
			</section>
			
			<!-- Network Information -->
			<section class="info-section">
				<h2>🌐 Network Information</h2>
				<div class="info-grid">
					<div class="info-item">
						<span class="label">Current Network:</span>
						<span class="value">{deviceInfo.currentNetwork}</span>
					</div>
					<div class="info-item">
						<span class="label">Gateway IP:</span>
						<span class="value">{deviceInfo.gatewayIp}</span>
					</div>
					<div class="info-item">
						<span class="label">Is TollGate:</span>
						<span class="value {deviceInfo.isTollgate ? 'success' : 'warning'}">
							{deviceInfo.isTollgate ? '✅ Yes' : '❌ No'}
						</span>
					</div>
				</div>
			</section>
			
			<!-- TollGate Information -->
			{#if deviceInfo.isTollgate && deviceInfo.tollgateInfo}
				<section class="info-section">
					<h2>🚪 TollGate Details</h2>
					<div class="info-grid">
						<div class="info-item">
							<span class="label">TollGate Pubkey:</span>
							<span class="value mono">{deviceInfo.tollgateInfo.tollgate_pubkey}</span>
						</div>
						<div class="info-item">
							<span class="label">Metric:</span>
							<span class="value">{deviceInfo.tollgateInfo.metric}</span>
						</div>
						<div class="info-item">
							<span class="label">Step Size:</span>
							<span class="value">{deviceInfo.tollgateInfo.step_size}</span>
						</div>
						<div class="info-item">
							<span class="label">Pricing Options:</span>
							<div class="pricing-options">
								{#each deviceInfo.tollgateInfo.pricing_options || [] as option}
									<div class="pricing-option">
										{option.steps} steps = {option.amount} sats
									</div>
								{/each}
							</div>
						</div>
					</div>
				</section>
			{/if}
			
			<!-- Wallet Information -->
			<section class="info-section">
				<h2>💰 Wallet Information</h2>
				<div class="info-grid">
					<div class="info-item">
						<span class="label">Balance:</span>
						<span class="value">{deviceInfo.walletBalance}</span>
					</div>
				</div>
			</section>
			
			<!-- Active Sessions -->
			<section class="info-section">
				<h2>🔄 Active Sessions</h2>
				{#if deviceInfo.activeSessions.length === 0}
					<p class="no-sessions">No active sessions</p>
				{:else}
					<div class="sessions-list">
						{#each deviceInfo.activeSessions as session}
							<div class="session-card">
								<div class="session-header">
									<span class="session-id">Session: {session.id}</span>
									<span class="session-status {session.status}">{session.status}</span>
								</div>
								<div class="session-details">
									<div class="session-detail">
										<span class="label">TollGate:</span>
										<span class="value mono">{session.tollgate_pubkey}</span>
									</div>
									<div class="session-detail">
										<span class="label">Usage:</span>
										<span class="value">{Math.round(session.usage_percentage * 100)}%</span>
									</div>
									<div class="session-detail">
										<span class="label">Remaining:</span>
										<span class="value">
											{#if session.metric === 'bytes'}
												{formatBytes(session.remaining_allotment)}
											{:else}
												{formatDuration(session.remaining_allotment)}
											{/if}
										</span>
									</div>
								</div>
							</div>
						{/each}
					</div>
				{/if}
			</section>
		</div>
		
		<div class="actions">
			<button on:click={loadDeviceInfo} class="refresh-btn">
				🔄 Refresh
			</button>
		</div>
	{/if}
</div>

<style>
	.developer-page {
		max-width: 800px;
		margin: 0 auto;
		padding: 20px;
	}
	
	h1 {
		text-align: center;
		margin-bottom: 30px;
		font-size: 28px;
		font-weight: 600;
	}
	
	.loading {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 20px;
		padding: 40px;
	}
	
	.spinner {
		width: 40px;
		height: 40px;
		border: 3px solid rgba(255, 255, 255, 0.3);
		border-top: 3px solid white;
		border-radius: 50%;
		animation: spin 1s linear infinite;
	}
	
	@keyframes spin {
		0% { transform: rotate(0deg); }
		100% { transform: rotate(360deg); }
	}
	
	.error {
		text-align: center;
		padding: 20px;
		background: rgba(255, 0, 0, 0.1);
		border: 1px solid rgba(255, 0, 0, 0.3);
		border-radius: 8px;
		margin-bottom: 20px;
	}
	
	.retry-btn {
		background: rgba(255, 255, 255, 0.2);
		border: 1px solid rgba(255, 255, 255, 0.3);
		color: white;
		padding: 8px 16px;
		border-radius: 6px;
		cursor: pointer;
		margin-top: 10px;
	}
	
	.info-sections {
		display: flex;
		flex-direction: column;
		gap: 20px;
	}
	
	.info-section {
		background: rgba(255, 255, 255, 0.1);
		border-radius: 12px;
		padding: 20px;
		backdrop-filter: blur(10px);
	}
	
	.info-section h2 {
		margin: 0 0 15px 0;
		font-size: 18px;
		font-weight: 600;
	}
	
	.info-grid {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}
	
	.info-item {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 8px 0;
		border-bottom: 1px solid rgba(255, 255, 255, 0.1);
	}
	
	.info-item:last-child {
		border-bottom: none;
	}
	
	.label {
		font-weight: 500;
		opacity: 0.8;
	}
	
	.value {
		font-weight: 600;
		text-align: right;
		max-width: 60%;
		word-break: break-all;
	}
	
	.value.mono {
		font-family: 'SF Mono', Monaco, 'Cascadia Code', 'Roboto Mono', Consolas, 'Courier New', monospace;
		font-size: 12px;
	}
	
	.value.success {
		color: #4ade80;
	}
	
	.value.warning {
		color: #fbbf24;
	}
	
	.pricing-options {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}
	
	.pricing-option {
		background: rgba(255, 255, 255, 0.1);
		padding: 4px 8px;
		border-radius: 4px;
		font-size: 12px;
	}
	
	.no-sessions {
		text-align: center;
		opacity: 0.6;
		font-style: italic;
		padding: 20px;
	}
	
	.sessions-list {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}
	
	.session-card {
		background: rgba(255, 255, 255, 0.05);
		border-radius: 8px;
		padding: 15px;
		border: 1px solid rgba(255, 255, 255, 0.1);
	}
	
	.session-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		margin-bottom: 10px;
	}
	
	.session-id {
		font-weight: 600;
		font-size: 14px;
	}
	
	.session-status {
		padding: 2px 8px;
		border-radius: 12px;
		font-size: 11px;
		font-weight: 600;
		text-transform: uppercase;
	}
	
	.session-status.active {
		background: rgba(34, 197, 94, 0.2);
		color: #4ade80;
	}
	
	.session-status.renewing {
		background: rgba(251, 191, 36, 0.2);
		color: #fbbf24;
	}
	
	.session-status.expired {
		background: rgba(239, 68, 68, 0.2);
		color: #f87171;
	}
	
	.session-details {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}
	
	.session-detail {
		display: flex;
		justify-content: space-between;
		font-size: 12px;
	}
	
	.actions {
		margin-top: 30px;
		text-align: center;
	}
	
	.refresh-btn {
		background: rgba(255, 255, 255, 0.2);
		border: 1px solid rgba(255, 255, 255, 0.3);
		color: white;
		padding: 12px 24px;
		border-radius: 8px;
		cursor: pointer;
		font-weight: 600;
		transition: all 0.2s ease;
	}
	
	.refresh-btn:hover {
		background: rgba(255, 255, 255, 0.3);
		transform: translateY(-1px);
	}
	
	@media (max-width: 600px) {
		.developer-page {
			padding: 15px;
		}
		
		.info-item {
			flex-direction: column;
			align-items: flex-start;
			gap: 4px;
		}
		
		.value {
			max-width: 100%;
			text-align: left;
		}
	}
</style>