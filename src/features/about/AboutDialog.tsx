// features/about/AboutDialog.tsx — About dialog: version, GitHub link, update check

import { useState, useEffect } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Dialog } from "../../components/ui/Dialog";
import { Button } from "../../components/ui/Button";
import { useI18n } from "../../lib/i18n";
import { useUpdater } from "../updater/useUpdater";
import { useUpdateStore } from "../../stores/updateStore";
import { APP_NAME, REPO_URL } from "../../lib/constants";

interface AboutDialogProps {
  open: boolean;
  onClose: () => void;
}

export function AboutDialog({ open, onClose }: AboutDialogProps) {
  const { t } = useI18n();
  const { checkForUpdate } = useUpdater();

  const [version, setVersion] = useState<string | null>(null);
  const [checking, setChecking] = useState(false);
  const [upToDate, setUpToDate] = useState(false);

  // Load app version when dialog first opens
  useEffect(() => {
    if (!open) return;
    getVersion().then(setVersion).catch(() => setVersion(null));
    // Reset transient state on each open
    setChecking(false);
    setUpToDate(false);
  }, [open]);

  const handleViewOnGitHub = () => {
    // Requires the opener capability `allow-open-url` + `allow-default-urls`.
    openUrl(REPO_URL).catch((err) => {
      console.error("Failed to open repository URL:", err);
    });
  };

  const handleCheckUpdates = async () => {
    setChecking(true);
    setUpToDate(false);
    await checkForUpdate();
    const status = useUpdateStore.getState().status;
    if (status === "available") {
      // An update was found — UpdateDialog takes over; close this dialog
      onClose();
    } else {
      setUpToDate(true);
    }
    setChecking(false);
  };

  return (
    <Dialog open={open} onClose={onClose} title={t("about.title")} width="400px">
      <div className="about-content">
        <p className="about-app-name">{APP_NAME}</p>
        <p className="about-description">{t("about.description")}</p>
        {version !== null && (
          <p className="about-version">
            {t("about.version", { version })}
          </p>
        )}

        {upToDate && (
          <p className="about-up-to-date">{t("about.upToDate")}</p>
        )}

        <div className="about-actions">
          <Button variant="secondary" onClick={handleViewOnGitHub}>
            {t("about.viewOnGitHub")}
          </Button>
          <Button
            variant="primary"
            onClick={handleCheckUpdates}
            disabled={checking}
          >
            {checking ? t("about.checking") : t("about.checkUpdates")}
          </Button>
        </div>
      </div>
    </Dialog>
  );
}
