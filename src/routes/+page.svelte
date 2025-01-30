<script lang="ts">
  import {
    ConnectionStatus
  } from "$lib/tollgate/types/ConnectionStatus";
  import {
    getTollgates
  } from "$lib/tollgate/network/helpers";
  import {onMount} from "svelte";

  import {fetch} from "@tauri-apps/plugin-http";
  import TollgateNetworkSession from "$lib/tollgate/network/TollgateNetworkSession";
  import type {Tollgate} from "$lib/tollgate/types/Tollgate";
  import type {NetworkInfo} from "$lib/tollgate/types/NetworkInfo";
  import {makePurchase} from "$lib/tollgate/purchase/renameme";

  import type IOperatingSystem from "$lib/os/IOperatingSystem";
  import AndroidOperatingSystem from "$lib/os/AndroidOperatingSystem";
  import MacOsOperatingSystem from "$lib/os/MacOsOperatingSystem";

  import type {ConnectedNetworkInfo} from "$lib/tollgate/types/ConnectedNetworkInfo";
  import { platform } from '@tauri-apps/plugin-os';


  let operatingSystem: IOperatingSystem;

  switch (platform()) {
    case "macos":
      operatingSystem = new MacOsOperatingSystem();
      break;
    case "android":
      operatingSystem = new AndroidOperatingSystem();
      break;
  }

  let userLog = $state([]);
  let purchaseMade = $state(false);

  let tollgates: Tollgate[] = $state([]);
  let networks: NetworkInfo[] = $state([]);

  let networkSession: TollgateNetworkSession | undefined = $state(undefined)
  let tollgateSession = $state(undefined)
  let currentNetwork: ConnectedNetworkInfo | undefined = $state(undefined);

  function log(str: string): void {
    userLog.push(str)
  }

  const runIntervalMs = 3000
  onMount(() => {
    const interval = setInterval(run, runIntervalMs);
    run();
    return () => clearInterval(interval);
  })

  let running = false;
  async function run(){

    if(running) return
    running = true;
    console.log("run");

    try {
      const currentNetworkTask = operatingSystem.getCurrentNetwork()
      const availableTollgatesTask = getAvailableTollgates()
      const macTask =  operatingSystem.getMacAddress(networkSession?.tollgate?.gatewayIp ?? "") // TODO: Error handling

      if(!networkSession){
        return;
      }

      const [currentNetworkResult, macResult, availableTollgatesResult] = await Promise.allSettled([
        currentNetworkTask,
        macTask,
        availableTollgatesTask
      ])

      tollgates = await availableTollgatesTask
      currentNetwork = await currentNetworkTask;
      const userMacAddress = await macTask;

      if(macResult.status === "fulfilled"){
        if(!userMacAddress){
          return;
        }
        networkSession.userMacAddress = userMacAddress;
      }

      if(!purchaseMade){
        await makePurchase(networkSession)
        purchaseMade = true;
      }

      let online = false
      await fetch(`https://api.cloudflare.com/client/v4/ips`, {connectTimeout: 150}).catch((reason) => {
        online = false;
      }).then((_) => {
        online = true;
      })
    } catch (e) {
      console.error("Running failed:", e);
    }

    // Connect to relay and pay
    running = false;
  }

  async function startTollgateSession(tollgate: Tollgate) {
    networkSession = new TollgateNetworkSession(tollgate);

    console.log("connecting to " + networkSession.tollgate.ssid);
    log("connecting to " + networkSession.tollgate.ssid);

    await operatingSystem.connectNetwork(networkSession.tollgate.ssid)
  }

  async function getAvailableTollgates() {
    networks = await operatingSystem.getAvailableNetworks()
    console.log(`found ${networks.length} networks`);

    return getTollgates(networks);
  }


</script>

{#await run()}{/await}

<main class="container">
  <h1>Welcome to Tollgate</h1>
  <h2>
    Network
    {#if (networkSession?.status === ConnectionStatus.connected)}
      <div style="color: green">CONNECTED</div>
    {:else if (networkSession?.status === ConnectionStatus.initiating)}
      <div style="color: chocolate">CONNECTING...</div>
    {:else}
      <div style="color: red">NOT CONNECTED</div>
    {/if}
  </h2>

  <h2>Current Network</h2>
  <table style="width:100%">
    <tbody>
    <tr>
      <td style="text-align: right"><strong>SSID</strong></td>
      <td style="text-align: left">{currentNetwork?.ssid}</td>
    </tr>
    <tr>
      <td style="text-align: right"><strong>My MAC address</strong></td>
      <td style="text-align: left">{networkSession?.userMacAddress}</td>
    </tr>
    <tr>
      <td style="text-align: right"><strong>Relay</strong></td>
        {#if (networkSession?.tollgateRelayReachable)}
          <td style="text-align: left"><div style="color: green">CONNECTED</div></td>
        {:else}
          <td style="text-align: left"><div style="color: red">NOT CONNECTED</div></td>
        {/if}
    </tr>
    </tbody>
  </table>

  <h2>Nearby tollgates</h2>
  <table style="width:100%">
    <tbody>
    <tr>
      <th><strong>SSID</strong></th>
<!--      <th><strong>BSSID</strong></th>-->
      <th><strong>Signal</strong></th>
      <th><strong>Freq</strong></th>
      <th><strong>Price</strong></th>
      <th><strong>Connect</strong></th>
    </tr>
    {#each tollgates as tollgate}
      <tr>
        <td>{tollgate.ssid}</td>
<!--        <td>{tollgate.bssid}</td>-->
        <td>{tollgate.rssi}</td>
        <td>{tollgate.frequency}</td>
        <td>{tollgate.pricing.allocationPer1024}/{tollgate.pricing.unit} - {tollgate.pricing.allocationType}</td>
        <td><button type="submit" onclick={() => startTollgateSession(tollgate)}>Connect</button></td>
      </tr>
    {/each}
    </tbody>
  </table>

  <h2>Logs</h2>
  <p>
  {#each userLog as log}
    {log}<br>
  {/each}
  </p>

</main>

<style>
.logo.vite:hover {
  filter: drop-shadow(0 0 2em #747bff);
}

.logo.svelte-kit:hover {
  filter: drop-shadow(0 0 2em #ff3e00);
}

:root {
  font-family: Inter, Avenir, Helvetica, Arial, sans-serif;
  font-size: 16px;
  line-height: 24px;
  font-weight: 400;

  color: #0f0f0f;
  background-color: #f6f6f6;

  font-synthesis: none;
  text-rendering: optimizeLegibility;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
  -webkit-text-size-adjust: 100%;
}

.container {
  margin: 0;
  padding-top: 10vh;
  display: flex;
  flex-direction: column;
  justify-content: center;
  text-align: center;
}

.logo {
  height: 6em;
  padding: 1.5em;
  will-change: filter;
  transition: 0.75s;
}

.logo.tauri:hover {
  filter: drop-shadow(0 0 2em #24c8db);
}

.row {
  display: flex;
  justify-content: center;
}

a {
  font-weight: 500;
  color: #646cff;
  text-decoration: inherit;
}

a:hover {
  color: #535bf2;
}

h1 {
  text-align: center;
}

input,
button {
  border-radius: 8px;
  border: 1px solid transparent;
  padding: 0.6em 1.2em;
  font-size: 1em;
  font-weight: 500;
  font-family: inherit;
  color: #0f0f0f;
  background-color: #ffffff;
  transition: border-color 0.25s;
  box-shadow: 0 2px 2px rgba(0, 0, 0, 0.2);
}

button {
  cursor: pointer;
}

button:hover {
  border-color: #396cd8;
}
button:active {
  border-color: #396cd8;
  background-color: #e8e8e8;
}

input,
button {
  outline: none;
}

#greet-input {
  margin-right: 5px;
}

@media (prefers-color-scheme: dark) {
  :root {
    color: #f6f6f6;
    background-color: #2f2f2f;
  }

  a:hover {
    color: #24c8db;
  }

  input,
  button {
    color: #ffffff;
    background-color: #0f0f0f98;
  }
  button:active {
    background-color: #0f0f0f69;
  }
}

</style>
