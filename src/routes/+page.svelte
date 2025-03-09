<script lang="ts">
  import { onMount} from "svelte";
  import { createFeedbackButton } from "nostr-feedback-button/feedback";
  import "nostr-feedback-button/styles.css";
  import {registerListener} from "$lib/tollgate/network/pluginCommands"
  import NetworkState, {type OnConnectedInfo} from "$lib/tollgate/network/NetworkState";
  import TollgateState from "$lib/tollgate/network/TollgateState";
  import { Subscription} from "rxjs";
  import {shortenString} from "$lib/util/helpers";
  import TollgateSession from "$lib/tollgate/network/TollgateSession";

  let userLog: string[] = $state([]);

  // let tollgates: Tollgate[] = $state([]);

  let networkState:  NetworkState = new NetworkState();
  let tollgateState:  TollgateState = new TollgateState(networkState);
  let tollgateSession:  TollgateSession = new TollgateSession(tollgateState);

  let macAddress = $state("?")
  let gatewayIp = $state("?")
  let networkHasRelay = $state(false)
  let relayActive = $state(false)
  let tollgateReady = $state(false)
  let tollgatePubkey = $state("?")
  let tollgateSessionActive = $state(false)

  $effect(() => {
    const subs: Subscription[] = []

    const vals: string[] = []

    subs.push(tollgateState._networkHasRelay.subscribe((value: boolean) => {
      console.log("networkHasRelay", value)
      networkHasRelay = value;
    }))

    subs.push(tollgateState._relayActive.subscribe((value: boolean) => {
      console.log("relayActive", value)
      relayActive = value;
    }))

    subs.push(tollgateState._tollgateIsReady.subscribe((value: boolean) => {
      console.log("tollgateReady", value)
      tollgateReady = value;
    }))

    subs.push(networkState._clientMacAddress.subscribe((value: string | undefined) => {
      console.log("macAddress", value)
      macAddress = value ?? "?";
    }))

    subs.push(networkState._gatewayIp.subscribe((value: string | undefined) => {
      console.log("gatewayIp", value)
      gatewayIp = value ?? "?";
    }))

    subs.push(tollgateState._tollgatePubkey.subscribe((value: string | undefined) => {
      console.log("tollgatePubkey", value)
      tollgatePubkey = value ?? "?";
    }))

    // subs.push(tollgateSession._sessionIsActive.subscribe((value: boolean) => {
    //   tollgateSessionActive = value;
    // }))

    return () => {
      subs.forEach(sub => sub.unsubscribe);
    }
  })

  onMount(async () => {
    await registerListener("network-connected", async () => {
      tollgateState._tollgateIsReady.subscribe(async (isReady: boolean) => {
        if(!isReady) {
          await networkState.performNetworkCheck()
        }
      })
    })

    await registerListener("network-disconnected", () => {
      networkState.reset()
    })

    networkState.networkIsReady.subscribe(async (networkIsReady) => {
      if(networkIsReady) await tollgateState.connect() // TODO: otherwise tollgateState.reset()
    })

    tollgateState._tollgateIsReady.subscribe(async (tollgateIsReady) => {
      if(tollgateIsReady) await tollgateSession.createSession()
    })
  })

  // creates and adds the feedback button to the page
  createFeedbackButton({
    developer: "1096f6be0a4d7f0ecc2df4ed2c8683f143efc81eeba3ece6daadd2fca74c7ecc",
    namespace: "tollgate-app",
    relays: [
      "wss://relay.damus.io",
    ],

    // additional options
  });

</script>

{#await networkState.performNetworkCheck()}{/await}

<main class="container">
  <h1>Welcome to Tollgate</h1>
<!--  <Wallet></Wallet>-->
  <h2>
    Session
    {#if (tollgateSessionActive)}
      <div style="color: green">ACTIVE</div>
    {:else}
      <div style="color: red">NOT ACTIVE</div>
    {/if}
  </h2>

  <h2>Current Network</h2>
  <table style="width:100%">
    <tbody>
    <tr>
      <td style="text-align: right"><strong>TollGate Ready</strong></td>
      <td style="text-align: left">
        {#if (tollgateReady)}
          <span style="color: green">YES </span>
        {:else}
          <span style="color: red">NO </span>
        {/if}
      </td>

    </tr>
    <tr>
      <td style="text-align: right"><strong>My MAC address</strong></td>
      <td style="text-align: left">{macAddress}</td>
    </tr>
    <tr>
      <td style="text-align: right"><strong>TollGate IP</strong></td>
      <td style="text-align: left">{gatewayIp}</td>
    </tr>
    <tr>
      <td style="text-align: right"><strong>TollGate PubKey</strong></td>
      <td style="text-align: left">{shortenString(tollgatePubkey, 5)}</td>
    </tr>
    <tr>
      <td style="text-align: right"><strong>Relay</strong></td>
          <td style="text-align: left">
            {#if (networkHasRelay)}
              <span style="color: green">YES </span><span> - </span>
            {:else}
              <span style="color: red">NO </span><span> - </span>
            {/if}
            {#if (relayActive)}
              <span style="color: green">CONNECTED</span>
            {:else}
              <span style="color: red">NOT CONNECTED</span>
            {/if}
          </td>
    </tr>
    </tbody>
  </table>

<!--  <h2>Nearby tollgates</h2>-->
<!--  <table style="width:100%">-->
<!--    <tbody>-->
<!--    <tr>-->
<!--      <th><strong>SSID</strong></th>-->
<!--&lt;!&ndash;      <th><strong>BSSID</strong></th>&ndash;&gt;-->
<!--      <th><strong>Signal</strong></th>-->
<!--      <th><strong>Freq</strong></th>-->
<!--      <th><strong>Price</strong></th>-->
<!--      <th><strong>Connect</strong></th>-->
<!--    </tr>-->
<!--    {#each tollgates as tollgate}-->
<!--      <tr>-->
<!--        <td>{tollgate.ssid}</td>-->
<!--&lt;!&ndash;        <td>{tollgate.bssid}</td>&ndash;&gt;-->
<!--        <td>{tollgate.rssi}</td>-->
<!--        <td>{tollgate.frequency}</td>-->
<!--        <td>{tollgate.pricing.allocationPer1024}/{tollgate.pricing.unit} - {tollgate.pricing.allocationType}</td>-->
<!--&lt;!&ndash;        <td><button type="button" onclick={() => startTollgateSession(tollgate)}>Connect</button></td>&ndash;&gt;-->
<!--      </tr>-->
<!--    {/each}-->
<!--    </tbody>-->
<!--  </table>-->

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
