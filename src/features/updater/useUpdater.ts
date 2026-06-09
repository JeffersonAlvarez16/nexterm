// features/updater/useUpdater.ts — Auto-update lifecycle hook
//
// Wraps @tauri-apps/plugin-updater and @tauri-apps/plugin-process.
// Provides checkForUpdate, downloadAndInstall, and dismissUpdate actions.
// Check errors are swallowed silently (REQ-1). Download errors go to store.

import { useCallback } from "react";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { useUpdateStore } from "../../stores/updateStore";
import type { UpdateInfo } from "../../stores/updateStore";

// Module-level so every useUpdater() instance shares the same pending update.
// A per-hook useRef breaks the flow: AboutDialog checks (storing the update in
// its own ref) but UpdateDialog installs from a different instance whose ref
// is empty, making "Update now" a silent no-op.
let pendingUpdate: Awaited<ReturnType<typeof check>> | null = null;

interface UseUpdater {
  checkForUpdate: () => Promise<void>;
  downloadAndInstall: () => Promise<void>;
  dismissUpdate: () => void;
}

/** Detects `[CRITICAL]` marker anywhere in the release notes body. */
function isCriticalUpdate(body: string | null | undefined): boolean {
  if (!body) return false;
  return body.includes("[CRITICAL]");
}

export function useUpdater(): UseUpdater {
  const { setStatus, setUpdateInfo, setProgress, setError, dismiss } =
    useUpdateStore();

  const checkForUpdate = useCallback(async () => {
    try {
      setStatus("checking");
      const update = await check();

      if (!update) {
        // No update available — go back to idle silently
        pendingUpdate = null;
        setStatus("idle");
        return;
      }

      pendingUpdate = update;

      const info: UpdateInfo = {
        version: update.version,
        body: update.body ?? "",
        date: update.date ?? new Date().toISOString(),
      };

      const critical = isCriticalUpdate(update.body);
      setUpdateInfo(info, critical);
    } catch {
      // REQ-1: Network failure during check MUST be silent
      setStatus("idle");
    }
  }, [setStatus, setUpdateInfo]);

  const downloadAndInstall = useCallback(async () => {
    const update = pendingUpdate;
    if (!update) return;

    try {
      setStatus("downloading");
      let totalBytes = 0;
      let downloadedBytes = 0;

      await update.downloadAndInstall((event) => {
        switch (event.event) {
          case "Started":
            totalBytes = event.data.contentLength ?? 0;
            downloadedBytes = 0;
            setProgress(0, totalBytes || null);
            break;
          case "Progress":
            downloadedBytes += event.data.chunkLength;
            setProgress(downloadedBytes, totalBytes || null);
            break;
          case "Finished":
            // Installation complete — will relaunch shortly
            break;
        }
      });

      setStatus("installing");
      await relaunch();
    } catch (err) {
      // REQ-3: Download failure shows error with retry option
      setError(err instanceof Error ? err.message : String(err));
    }
  }, [setStatus, setProgress, setError]);

  const dismissUpdate = useCallback(() => {
    dismiss();
  }, [dismiss]);

  return {
    checkForUpdate,
    downloadAndInstall,
    dismissUpdate,
  };
}
