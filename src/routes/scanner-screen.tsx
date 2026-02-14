import { useEffect, useState } from "react";
import {
  cancel,
  scan,
  Format,
  checkPermissions,
  requestPermissions,
  openAppSettings,
} from "@tauri-apps/plugin-barcode-scanner";
import type { PermissionState } from "@tauri-apps/api/core";
import { Settings } from "lucide-react";
import { Screen } from "@/components/layout/screen";
import { Button } from "@/components/ui/button";

type ScannerPageProps = {
  onResult: (content: string) => void;
  onCancel: () => void;
};

export function ScannerPage({ onResult, onCancel }: ScannerPageProps) {
  const [error, setError] = useState<string | null>(null);
  const [permissionState, setPermissionState] =
    useState<PermissionState | null>(null);

  useEffect(() => {
    let active = true;

    const initScanner = async () => {
      try {
        let status: PermissionState = await checkPermissions();

        // If not granted, request it directly
        // 'prompt' and 'prompt-with-rationale' fall here
        if (status === "prompt" || (status as string) === "prompt-with-rationale") {
          const requestResult = await requestPermissions();
          status = requestResult;
        }

        // If explicitly denied, we must go to settings
        if (status === "denied") {
          setPermissionState(status);
          setError(
            "Camera permission denied. Please enable it in settings to use the scanner.",
          );
          return;
        }

        setPermissionState(status);

        if (status === "granted") {
          const result = await scan({
            windowed: true,
            formats: [Format.QRCode],
          });

          if (active && result.content) {
            onResult(result.content);
          }
        } else {
          // It might be denied now or the user dismissed the prompt
          setError("Camera permission is required to scan QR codes.");
        }
      } catch (err) {
        console.error("Scanner error:", err);
        // Sometimes the plugin throws if permissions are messed up, try to handle gracefully
        if (active) setError("Failed to initialize scanner.");
      }
    };

    initScanner();

    // Force transparency on body and root for this screen
    document.body.style.backgroundColor = "transparent";
    document.documentElement.style.backgroundColor = "transparent";
    const root = document.getElementById("root");
    if (root) root.style.backgroundColor = "transparent";

    return () => {
      active = false;
      cancel();
      // Revert transparency
      document.body.style.backgroundColor = "";
      document.documentElement.style.backgroundColor = "";
      if (root) root.style.backgroundColor = "";
    };
  }, [onResult]);

  return (
    <Screen className="bg-transparent">
      <div className="relative h-full w-full">
        <div className="absolute top-4 left-4 z-50 flex gap-2">
          {permissionState === "denied" && (
            <Button
              variant="outline"
              size="icon"
              className="h-12 w-12 rounded-full bg-background/80 backdrop-blur-sm"
              onClick={() => openAppSettings()}
            >
              <Settings className="h-6 w-6" />
            </Button>
          )}
        </div>

        <div className="flex h-full flex-col items-center justify-center">
          {permissionState === "granted" && !error ? (
            <>
              <div className="h-64 w-64 rounded-xl border-2 border-white/50 box-content relative">
                <div className="absolute top-0 left-0 w-4 h-4 border-t-2 border-l-2 border-primary -translate-x-1 -translate-y-1" />
                <div className="absolute top-0 right-0 w-4 h-4 border-t-2 border-r-2 border-primary translate-x-1 -translate-y-1" />
                <div className="absolute bottom-0 left-0 w-4 h-4 border-b-2 border-l-2 border-primary -translate-x-1 translate-y-1" />
                <div className="absolute bottom-0 right-0 w-4 h-4 border-b-2 border-r-2 border-primary translate-x-1 translate-y-1" />
              </div>
              <p className="mt-8 text-white font-medium bg-black/50 px-4 py-2 rounded-full backdrop-blur-sm text-center">
                Center the QR code in the frame
              </p>
            </>
          ) : null}

          {error && (
            <div className="mt-4 p-6 bg-background/95 backdrop-blur-md rounded-2xl mx-4 text-center shadow-xl border border-border max-w-sm">
              <h3 className="text-lg font-semibold mb-2">Camera Access</h3>
              <p className="text-muted-foreground mb-6">{error}</p>

              <div className="space-y-3">
                {permissionState === "denied" && (
                  <Button onClick={() => openAppSettings()} className="w-full">
                    Open Settings
                  </Button>
                )}
                <Button variant="outline" onClick={onCancel} className="w-full">
                  Go Back
                </Button>
              </div>
            </div>
          )}
        </div>
      </div>
    </Screen>
  );
}
