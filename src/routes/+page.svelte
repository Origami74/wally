<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import {Buffer} from "buffer";
  import {u} from "../../.svelte-kit/output/server/chunks";

  let name = $state("");
  let greetMsg = $state("");

  async function greet(event: Event) {
    event.preventDefault();
    greetMsg = await invoke("greet", { name });
  }

  interface NetworkElement {
    id: string;
    idExt: string;
    bytes: string[]; // TODO: wrong type?
  }

  interface NetworkInfo {
    ssid: string;
    bssid: string;
    rssi: number; // signal strenght in dB
    capabilities: string;
    frequency: string;
    informationElements: NetworkElement[];
  }

  // Checks if the passed array matches the tollgate vendor_elements bytes (212121).
  // This is useful to avoid having to parse everything from hex to string first.
  function getTollgateVendorElement(network: NetworkInfo): NetworkElement | undefined {
    const tollgateIdentifierBytes = ["50","49","50","49","50","49"]

    for (const element of network.informationElements) {

      const x = element.bytes.slice(0, 6);
      if(tollgateIdentifierBytes.every((val, index) => val == x[index])){
        return element;
      }
    }

    return undefined;
  }

  function isTollgate(network: NetworkInfo): boolean {

    // All tollgates have to identify as tollgate (openwrt for debugging purposes)
    if(!network.ssid.toLowerCase().startsWith("tollgate") && !network.ssid.toLowerCase().startsWith("openwrt")) {
      return false;
    }

    // Check if any of the information elements contains the tollgate info we're looking for

    if(getTollgateVendorElement(network) != undefined) {
      return true
    }

    console.log(`network ${name} does not contain TollGate element`);
    return false;
  }

  interface TollgatePricing {
    allocationType: string;
    allocationPer1024: number;
    unit: string;
  }

  interface Tollgate {
    ssid: string;
    bssid: string;
    rssi: number;
    frequency: string;
    version: string;
    pubkey: string;
    pricing: TollgatePricing
  }

  let tollgates: Tollgate[] = $state([]);
  let networks: NetworkInfo[] = $state([]);

  function toTollgate(network: NetworkInfo) {
    const vendorElements = hexDecode(getTollgateVendorElement(network).bytes)


    const tollgateInfo = vendorElements
            .slice(8) // drop vendor identifier
            .split('|');

    const tollgate: Tollgate = {
      ssid: network.ssid,
      bssid: network.bssid,
      rssi: network.rssi,
      frequency: network.frequency,
      pubkey: tollgateInfo[1],
      version: tollgateInfo[0],
      pricing: {
        allocationType: tollgateInfo[2],
        allocationPer1024: tollgateInfo[3],
        unit: tollgateInfo[4],
      },
    }

    return tollgate;
  }

  async function getWifiDetails() {
    let response = await invoke("plugin:androidwifi|getWifiDetails", { payload: { value: "" } });
    networks = JSON.parse(response.wifis);
    console.log(`found ${networks.length} networks`);

    let tollgateNetworks: any[] = $state([]);
    networks.forEach(network => {
      if(!isTollgate(network)) {
        return;
      }

      console.log(`Network ${network.ssid} is a tollgate!`);
      tollgateNetworks.push(network);
    })

    tollgateNetworks.forEach(network => {
      tollgates.push(toTollgate(network))
    })
  }


  function hexDecode(hex: string): string {
    return Buffer.from(hex, 'hex').toString()
  }


</script>

{#await getWifiDetails()}{/await}

<main class="container">
  <h1>Welcome to Tauri + Svelte</h1>

  <div class="row">
    <a href="https://vitejs.dev" target="_blank">
      <img src="/vite.svg" class="logo vite" alt="Vite Logo" />
    </a>
    <a href="https://tauri.app" target="_blank">
      <img src="/tauri.svg" class="logo tauri" alt="Tauri Logo" />
    </a>
    <a href="https://kit.svelte.dev" target="_blank">
      <img src="/svelte.svg" class="logo svelte-kit" alt="SvelteKit Logo" />
    </a>
  </div>
  <p>Click on the Tauri, Vite, and SvelteKit logos to learn more.</p>

  {#each tollgates as tollgate}
      <h4>Tollgate</h4>
    <table style="width:100%">
      <tbody>
      <tr>
        <td style="text-align: right"><strong>SSID</strong></td>
        <td style="text-align: left">{tollgate.ssid}</td>
      </tr>
      <tr>
        <td style="text-align: right"><strong>BSSID</strong></td>
        <td style="text-align: left">{tollgate.bssid}</td>
      </tr>
      <tr>
        <td style="text-align: right"><strong>Signal</strong></td>
        <td style="text-align: left">{tollgate.rssi} dB</td>
      </tr>
      <tr>
        <td style="text-align: right"><strong>Frequency</strong></td>
        <td style="text-align: left">{tollgate.frequency} Mhz</td>
      </tr>
      <tr>
        <td style="text-align: right"><strong>TollGate version</strong></td>
        <td style="text-align: left">{tollgate.version}</td>
      </tr>
      <tr>
        <td style="text-align: right"><strong>Nostr pubkey</strong></td>
        <td style="text-align: left">{tollgate.pubkey.slice(0, 7)}...{tollgate.pubkey.slice(57)}</td>
      </tr>
      <tr>
        <td style="text-align: right"><strong>Price</strong></td>
        <td style="text-align: left">{1024/tollgate.pricing.allocationPer1024} {tollgate.pricing.unit} per {tollgate.pricing.allocationType}</td>
      </tr>
      </tbody>
    </table>
  {/each}

  <form class="row" onsubmit={greet}>
    <input id="greet-input" placeholder="Enter a name..." bind:value={name} />
    <button type="submit">Greet</button>
  </form>
  <p>{greetMsg}</p>
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
