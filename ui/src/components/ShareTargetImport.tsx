import { useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import ImportMappingDialog from "./ImportMappingDialog";
import { uploadCsv } from "../api/csvUpload";
import { isServerMode } from "../api/auth";
import { clearShareFlag, readShareFlag, takeSharedFile, MAX_SHARE_MB } from "../pwa/shareTarget";
import { userErrorMessage } from "../utils/runtime";

/**
 * Completes an OS share-sheet hand-off: picks up the CSV the service worker
 * parked in IndexedDB and drops the user straight into the normal import
 * mapping dialog.
 *
 * Mounted app-wide (App.tsx) because a share launch can land on ANY route — the
 * PWA reopens wherever it last was, and the `?shared=` flag rides along.
 *
 * No `defaultAccountId` is passed: a share arrives with no account context, so
 * the dialog's own account picker is the right place to choose. It already
 * blocks submission until one is selected.
 */
export default function ShareTargetImport() {
  const [path, setPath] = useState<string | null>(null);
  // Guards the destructive read. `takeSharedFile` consumes the record, and
  // StrictMode runs effects twice in dev — without this the second pass finds
  // nothing and reports a spurious failure.
  const handled = useRef(false);

  useEffect(() => {
    if (handled.current) return;

    const outcome = readShareFlag();
    if (outcome === "none") return;

    handled.current = true;
    // Drop the flag immediately: a refresh must not re-enter this path.
    clearShareFlag();

    if (outcome === "empty") {
      toast.error("Nothing to import", {
        description: "That share didn't include a file, or the file was empty.",
      });
      return;
    }
    if (outcome === "toolarge") {
      toast.error("That file is too large", {
        description: `FinSight imports statements up to ${MAX_SHARE_MB} MB. Try exporting a shorter date range.`,
      });
      return;
    }
    if (outcome === "unsupported") {
      toast.error("FinSight imports CSV statements", {
        description:
          "That file isn't a CSV. Most banks offer a CSV or spreadsheet export alongside the PDF.",
      });
      return;
    }
    if (outcome === "error") {
      toast.error("Couldn't receive that file", { description: "Try sharing it again." });
      return;
    }

    // The upload needs the authenticated first-party session, which only exists
    // in server/PWA mode. In the Tauri shell the share sheet never reaches us.
    if (!isServerMode()) return;

    void (async () => {
      const file = await takeSharedFile();
      if (!file) {
        toast.error("That shared file is no longer available", {
          description: "Try sharing it to FinSight again.",
        });
        return;
      }

      const pending = toast.loading(`Receiving ${file.name}…`);
      try {
        setPath(await uploadCsv(file));
        toast.dismiss(pending);
      } catch (err) {
        toast.dismiss(pending);
        toast.error("Couldn't upload that file", { description: userErrorMessage(err) });
      }
    })();
  }, []);

  if (!path) return null;

  return (
    <ImportMappingDialog
      path={path}
      onClose={() => setPath(null)}
      onImported={() => setPath(null)}
    />
  );
}
