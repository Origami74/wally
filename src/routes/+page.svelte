<script lang="ts">
  import {ConnectionStatus} from "$lib/tollgate/types/ConnectionStatus";
  import {getTollgates, isTollgateSsid} from "$lib/tollgate/network/helpers";
  import {onMount} from "svelte";

  import {fetch} from "@tauri-apps/plugin-http";
  import TollgateNetworkSession from "$lib/tollgate/network/TollgateNetworkSession";
  import type {Tollgate} from "$lib/tollgate/types/Tollgate";
  import type {NetworkInfo} from "$lib/tollgate/types/NetworkInfo";
  import {makePurchase} from "$lib/tollgate/purchase/renameme";

  import type {ConnectedNetworkInfo} from "$lib/tollgate/types/ConnectedNetworkInfo";
  import { getAvailableNetworks, getCurrentNetwork, connectNetwork, getMacAddress } from "$lib/tollgate/network/pluginCommands"

  let userLog = $state([]);
  let purchaseMade = $state(false);
  let online = $state(false);
  let relayReachableView = $state(false);

  let tollgates: Tollgate[] = $state([]);
  let networks: NetworkInfo[] = $state([]);

  let networkSession: TollgateNetworkSession | undefined = $state(undefined)
  let tollgateSession = $state(undefined)
  let currentNetwork: ConnectedNetworkInfo | undefined = $state(undefined);


  let networkStatusView = $state(ConnectionStatus.disconnected)

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
    // console.log(`running: ${running}`);

    if(running) return
    running = true;

    console.log("mac2");
    var mac2 = await  getMacAddress(networkSession?.tollgate?.gatewayIp)
    console.log(`mac2 finish: ${mac2}`);

    try {
      const currentNetworkTask = getCurrentNetwork()
      const availableTollgatesTask = getAvailableTollgates()
      const macTask =  getMacAddress(networkSession?.tollgate?.gatewayIp) // TODO: Error handling

      const [currentNetworkResult, macResult, availableTollgatesResult] = await Promise.allSettled([
        currentNetworkTask,
        macTask,
        availableTollgatesTask
      ])

      console.log(`currentNetwork/mac/availableTollgates: ${currentNetworkResult.status}/${macResult.status}/${availableTollgatesResult.status}`)

      if(currentNetworkResult.status === "fulfilled"){
        currentNetwork = await currentNetworkTask;
      }

      if(availableTollgatesResult.status === "fulfilled"){
        tollgates = await availableTollgatesTask
        console.log(tollgates)
      }

      if(macResult.status === "fulfilled"){
        const userMacAddress = await macTask;
        if(networkSession && userMacAddress != undefined){
          networkSession.userMacAddress = userMacAddress;
        }
      }

      // console.log("networkSession", JSON.stringify(networkSession));
      if(!networkSession){

        // if we're already connected, make a session
        if(isTollgateSsid(currentNetwork?.ssid ?? "null")){
          const currentTollgate = tollgates.find(tg => tg.ssid === currentNetwork?.ssid);
          if(currentTollgate){
            await startTollgateSession(currentTollgate)
          }

        }

        running = false;
        return;
      }

      networkStatusView = ConnectionStatus.initiating;

      if(networkSession.userMacAddress === undefined){
        console.log("waiting for userMacAddress");
        running = false;
        return;
      }

      if(!networkSession.tollgateRelayReachable){
        console.log("waiting for tollgateRelayReachable");
        const relay = networkSession!.tollgateRelay
        running = false;
        return;
      }

      console.log(`RELAY REACHABLE! purchaseMade=${purchaseMade}`);
      relayReachableView = true

      if(!purchaseMade){
        console.log("starting makePurchase");
        await makePurchase(networkSession)
        purchaseMade = true;
        running = false;
        return;
      }

      if(online){
        networkStatusView = ConnectionStatus.connected;
        running = false;
        return;
      }

      online = true;
      try{
        var response = await fetch(`https://api.cloudflare.com/client/v4/ips`, {connectTimeout: 150})

        if(!response){
          console.log("--noe response--")
          online = false;
        }

        console.log("are we online?:", );
        online = true;
      }
      catch(error) {
        online = false;
        console.log("--offline--")
      }

    } catch (e) {
      running = false;
      console.error("Running failed:", e);
    }

    // Connect to relay and pay
    running = false;
  }

  async function startTollgateSession(tollgate: Tollgate) {
    if(networkSession?.tollgate.ssid === tollgate.ssid){
      console.log(`Already connected to tollgate ${tollgate.ssid}`);
    }

    networkSession = new TollgateNetworkSession(tollgate);

    console.log("connecting to " + networkSession.tollgate.ssid);
    log("connecting to " + networkSession.tollgate.ssid);
    console.log("networkSession: ", JSON.stringify(networkSession))

    if(networkSession.tollgate.ssid === currentNetwork?.ssid){
      console.log(`already connected to ${currentNetwork.ssid}, not switching`);
      return
    }
    await connectNetwork(networkSession.tollgate.ssid)
  }

  async function getAvailableTollgates() {
    networks = await getAvailableNetworks()
    // console.log(`found ${networks.length} networks`);

    return getTollgates(networks);
  }


</script>

{#await run()}{/await}

<main class="container">
  <h1>Welcome to Tollgate</h1>
  <h2>
    Network
    {#if (networkStatusView === ConnectionStatus.connected)}
      <div style="color: green">CONNECTED</div>
    {:else if (networkStatusView === ConnectionStatus.initiating)}
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
      <td style="text-align: right"><strong>TollGate IP</strong></td>
      <td style="text-align: left">{networkSession?.tollgate.gatewayIp}</td>
    </tr>
    <tr>
      <td style="text-align: right"><strong>Relay</strong></td>
        {#if (relayReachableView)}
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
