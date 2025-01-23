<script lang="ts">
  import {invoke} from "@tauri-apps/api/core";
  import {ConnectionStatus, type NetworkInfo, type Tollgate} from "$lib/tollgate/ConnectionStatus";
  import {
    getMacAddress,
    isTollgateNetwork,
    isTollgateSsid, toTollgate
  } from "$lib/tollgate/helpers";
  import {onMount} from "svelte";

  let connectionStatus = $state(ConnectionStatus.disconnected);
  let ssid = $state("");
  let mac = $state("");
  let macNative = $state("");
  let userLog = $state([]);

  let tollgates: Tollgate[] = $state([]);
  let networks: NetworkInfo[] = $state([]);

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
    console.log("run");
    if(running){
      return;
    }

    running = true;
    try{
      // networks = []
      // tollgates = []

      const wifiDetailsTask = invoke("plugin:androidwifi|getCurrentWifiDetails", { })
      const macTask =  getMacAddress()
      const [wifiDetailsResult, macResult] = await Promise.allSettled([
        wifiDetailsTask,
        macTask
      ])

      if(macResult.status === "fulfilled"){
        mac = await macTask;
      } else {
        console.error("could not get mac")
        mac = "n/a"
      }

      const details = JSON.parse((await wifiDetailsTask).wifiDetails) // TODO: get just the object instead of nested?
      ssid = details.ssid.replaceAll('"',''); // TODO: bug in serialization from android

      if(!isTollgateSsid(ssid)){
        connectionStatus = ConnectionStatus.disconnected
      }

      connectionStatus = ConnectionStatus.initiating

    } catch (e) {
      console.error("Running failed:", e);
    }


    // Connect to relay and pay
    running = false;
  }

  async function connectNetwork(ssid: string) {
    console.log("connecting to " + ssid);
    log("connecting to " + ssid);
    let response = await invoke("plugin:androidwifi|connectWifi", { ssid: ssid });
    log("response for connecting to " + ssid + " = " + JSON.stringify(response));
  }

  async function getWifiDetails() {
    let response = await invoke("plugin:androidwifi|getWifiDetails", { payload: { value: "" } });
    networks = JSON.parse(response.wifis);
    console.log(`found ${networks.length} networks`);

    let tollgateNetworks: any[] = $state([]);
    networks.forEach(network => {
      if(!isTollgateNetwork(network)) {
        return;
      }

      console.log(`Network ${network.ssid} is a tollgate!`);
      tollgateNetworks.push(network);
    })

    tollgateNetworks.forEach(network => {
      tollgates.push(toTollgate(network))
    })
  }


</script>

{#await Promise.all([getWifiDetails(), run()])}{/await}

<main class="container">
  <h1>Welcome to Tollgate</h1>
  <h2>
    You are
    {#if (connectionStatus == ConnectionStatus.connected)}
      <div style="color: green">CONNECTED</div>
    {:else if (connectionStatus == ConnectionStatus.initiating)}
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
      <td style="text-align: left">{ssid}</td>
    </tr>
    <tr>
      <td style="text-align: right"><strong>MAC address</strong></td>
      <td style="text-align: left">{mac}</td>
    </tr>
    <tr>
      <td style="text-align: right"><strong>MAC native</strong></td>
      <td style="text-align: left">{macNative}<button type="submit" onclick={() => getMacNative()}>Connect</button></td>
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
        <td><button type="submit" onclick={() => connectNetwork(tollgate.ssid)}>Connect</button></td>
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
