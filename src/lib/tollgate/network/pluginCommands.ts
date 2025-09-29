import { invoke } from "@tauri-apps/api/core";
import { Channel, PluginListener } from "@tauri-apps/api/core";

export async function registerListener(
    eventName: string,
    onEvent: (data: any) => void,
): Promise<PluginListener> {
    const handler = new Channel();
    handler.onmessage = async (data: any) => {
        // Handle network events and forward to Rust backend
        if (eventName === "network-connected") {
            try {
                // Get network details and forward to Rust
                const gatewayIp = await getGatewayIp();
                const macAddress = await getClientMacAddress(gatewayIp);
                
                if (gatewayIp && macAddress) {
                    await invoke("handle_network_connected", {
                        gatewayIp,
                        macAddress
                    });
                }
            } catch (error) {
                console.error("Error handling network connected:", error);
            }
        } else if (eventName === "network-disconnected") {
            try {
                await invoke("handle_network_disconnected");
            } catch (error) {
                console.error("Error handling network disconnected:", error);
            }
        }
        
        // Also call the original handler
        onEvent(data);
    };
    
    return invoke("plugin:androidwifi|register_listener", { event: eventName, handler }).then(
        () => new PluginListener("androidwifi", eventName, handler.id)
    );
}

export async function getClientMacAddress(gatewayIp: string | undefined): Promise<string | undefined> {
    if (gatewayIp === undefined) {
        return undefined;
    }

    try {
        const macAddressResult: { macAddress: string | undefined } = await invoke("plugin:androidwifi|get_mac_address", { 
            payload: { gatewayIp: gatewayIp } 
        });

        let macAddress = macAddressResult.macAddress;

        // Convert 'null' string back to undefined
        if (macAddress === null || macAddress === 'null') {
            macAddress = undefined;
        }

        return macAddress;
    } catch (e) {
        throw new Error(`Failed to determine MAC address, reason: ${e}`);
    }
}

export async function getGatewayIp(): Promise<string | undefined> {
    try {
        const gatewayIpResult: { gatewayIp: string | undefined } = await invoke("plugin:androidwifi|get_gateway_ip", { 
            payload: {} 
        });

        let gatewayIp = gatewayIpResult.gatewayIp;

        // Convert 'null' string back to undefined
        if (gatewayIp === null || gatewayIp === 'null') {
            gatewayIp = undefined;
        }

        return gatewayIp;
    } catch (e) {
        throw new Error(`Failed to determine gatewayIp, reason: ${e}`);
    }
}

export async function getCurrentNetwork() {
    const currentNetworkInfo = await invoke("plugin:androidwifi|get_current_wifi_details", { 
        payload: { value: "" } 
    });
    return currentNetworkInfo.wifi;
}

export async function getAvailableNetworks() {
    try {
        let response = await invoke("plugin:androidwifi|get_wifi_details", { 
            payload: { value: "" } 
        });
        return response.wifis;
    } catch (e) {
        console.error(`Failed to perform network scan, reason: ${e}`);
        return [];
    }
}

export async function connectNetwork(ssid: string): Promise<void> {
    try {
        let response = await invoke("plugin:androidwifi|connect_wifi", { 
            payload: { ssid: ssid } 
        });
        console.log("response for connecting to " + ssid + " = " + JSON.stringify(response));
    } catch (e) {
        console.error(`Error connecting to network ${ssid}, reason: ${e}`);
    }
}

export async function markCaptivePortalDismissed(): Promise<void> {
    try {
        await invoke("plugin:androidwifi|mark_captive_portal_dismissed", { 
            payload: {} 
        });
    } catch (e) {
        console.error(`Error marking captive portal dismissed, reason: ${e}`);
    }
}
