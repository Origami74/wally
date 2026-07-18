#!/bin/bash
set -e

echo "🚀 Starting APK Build Process for Wally..."

# Ensure we are in the project root
if [ ! -f "package.json" ]; then
    echo "❌ Error: package.json not found. Please run this script from the project root."
    exit 1
fi

# Install dependencies
echo "📦 Installing dependencies..."
pnpm install

# Initialize Android project
echo "🤖 Initializing Android project..."
npx tauri android init

# Build the APK
echo "🔨 Building Android APK..."
export TAURI_ANDROID_KEYSTORE=$PWD/src-tauri/release.keystore
export TAURI_ANDROID_KEYSTORE_PASSWORD=password
export TAURI_ANDROID_KEY_ALIAS=release-key
export TAURI_ANDROID_KEY_PASSWORD=password
npx tauri android build --apk true

# Define paths
RELEASE_DIR="src-tauri/gen/android/app/build/outputs/apk/universal/release"
SOURCE_SIGNED="$RELEASE_DIR/app-universal-release.apk"
SOURCE_UNSIGNED="$RELEASE_DIR/app-universal-release-unsigned.apk"
DEST_DIR="releases"
DEST_FILE="$DEST_DIR/wally-release.apk"

# Check if build succeeded and move file
if [ -f "$SOURCE_SIGNED" ]; then
    mkdir -p "$DEST_DIR"
    cp "$SOURCE_SIGNED" "$DEST_FILE"
    echo "✅ Success! Signed APK is ready."
    echo "📍 Location: $DEST_FILE"
elif [ -f "$SOURCE_UNSIGNED" ]; then
    mkdir -p "$DEST_DIR"
    cp "$SOURCE_UNSIGNED" "$DEST_FILE"
    echo "⚠️  Success! Unsigned APK is ready (signing config missing)."
    echo "📍 Location: $DEST_FILE"
else
    echo "❌ Build failed. APK not found at expected location: $RELEASE_DIR"
    exit 1
fi
