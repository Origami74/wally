# TollGate - App

This is a multiplatform app that allows your device to discover and (auto)connect to TollGates around you.

The current version is focussing on Android first. Support for Linux and MacOS will be next.

## Building for Android

```bash
# MacOS
export ANDROID_HOME="$HOME/Library/Android/sdk"
export NDK_HOME="$ANDROID_HOME/ndk/$(ls -1 $ANDROID_HOME/ndk)"

pnpm tauri android init

pnpm tauri android dev
```


In MacOS:
```shell
export ANDROID_HOME="$HOME/Library/Android/sdk"
export NDK_HOME="$ANDROID_HOME/ndk/$(ls -1 $ANDROID_HOME/ndk)"
export JAVA_HOME="/Users/[username]/Applications/Android Studio.app/Contents/jbr/Contents/Home"
```
