# TollGate - App

This is a multiplatform app that allows your device to discover and (auto)connect to TollGates around you.

The current version is focussing on Android first. Support for Linux and MacOS will be next.

## Cashu Wallet Connect (CWC)

This app implements **Cashu Wallet Connect (CWC)**, an extension of [NIP-47 (Nostr Wallet Connect)](https://github.com/nostr-protocol/nips/blob/master/47.md) that enables applications to interact with Cashu ecash wallets over Nostr. 

**📖 Read the [CWC Specification](SPECIFICATION.md)** for full protocol details.

### Key Features

- 🔌 **NWC Compatible**: Implements NIP-47 with Cashu extensions
- 🪙 **Multi-Mint Support**: Manage balances across multiple Cashu mints
- ⚡ **Lightning Bridge**: Pay/receive Lightning invoices via mint gateways
- 🔐 **Budget Controls**: Per-connection spending limits
- 🎯 **Cashu Native**: Direct token sending/receiving with NUT-18 support

## Wallet Public Key Access

The wallet exposes its Nostr public key via a local HTTP server on port **3737**. This allows other services running on the same device to easily retrieve the wallet's public key in hex format.

### Endpoints

- **GET http://127.0.0.1:3737** - Returns the wallet's Nostr public key in hex format
  ```json
  {
    "pubkey": "abc123...",
    "success": true
  }
  ```

### Example Usage

```bash
# Get the wallet's public key
curl http://127.0.0.1:3737
```

## Building for Desktop

```bash
pnpm i

pnpm tauri dev
```

## Building for Android

```bash
# MacOS - Android Development Environment
export ANDROID_HOME="$HOME/Library/Android/sdk"
export ANDROID_NDK_HOME="$ANDROID_HOME/ndk/$(ls -1 $ANDROID_HOME/ndk)"
export NDK_HOME="$ANDROID_NDK_HOME"
export PATH="$PATH:$ANDROID_HOME/tools:$ANDROID_HOME/platform-tools"

# Java 17 for Android compatibility (use SDKMAN)
# sdk use java 17.0.16-amzn
export JAVA_HOME="$HOME/.sdkman/candidates/java/17.0.16-amzn"

pnpm tauri android init

pnpm tauri android dev
```


In MacOS:
```shell
export ANDROID_HOME="$HOME/Library/Android/sdk"
export NDK_HOME="$ANDROID_HOME/ndk/$(ls -1 $ANDROID_HOME/ndk)"
export JAVA_HOME="/Users/[username]/Applications/Android Studio.app/Contents/jbr/Contents/Home"
```

## License
This project is licensed under the GNU General Public License v3.0 - see the [LICENSE](LICENSE) file for details.
