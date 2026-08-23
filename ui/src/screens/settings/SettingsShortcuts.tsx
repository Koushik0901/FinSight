import { useState, useEffect } from "react";
import { toast } from "sonner";
import { Section } from "./Section";
import PushNotificationSettings from "../../components/PushNotificationSettings";
import NotificationPolicySettings from "../../components/NotificationPolicySettings";
import { fetchServerAbout, type ServerAbout, CLIENT_PROTOCOL } from "../../api/serverInfo";
import { isServerMode } from "../../api/auth";
import {
  useSimpleFinStatus,
  useDisconnectSimpleFin,
  usePurgeSimpleFinData,
  useSimpleFinConnections,
  useDeleteSimpleFinConnection,
  useSimpleFinSyncSettings,
  useSetSimpleFinSyncSettings,
} from "../../api/hooks/simplefin";
import SimpleFinDialog from "../onboarding/SimpleFinDialog";
import McpConnectionsSection from "../../components/McpConnectionsSection";

function ConnectionsSection() {
  const { data: sfStatus } = useSimpleFinStatus();
  const { data: sfConnections = [] } = useSimpleFinConnections();
  const disconnectSf = useDisconnectSimpleFin();
  const purgeSf = usePurgeSimpleFinData();
  const deleteConnection = useDeleteSimpleFinConnection();
  const { data: sfSyncSettings } = useSimpleFinSyncSettings();
  const setSfSyncSettings = useSetSimpleFinSyncSettings();
  const [sfDialogOpen, setSfDialogOpen] = useState(false);
  const serverModeConn = isServerMode();

  return (
    <Section id="connections" title="Connections" description="Bank feeds, background sync, and external AI assistants.">
      <div className="s-row">
        <div>
          <div className="label">SimpleFin</div>
          <div className="desc">Connect or add institutions and import synced transactions.</div>
        </div>
        <div className="muted">{sfStatus?.configured ? "Connected" : "Not connected"}</div>
        <button className="btn sm" type="button" onClick={() => setSfDialogOpen(true)}>
          {sfStatus?.configured ? "Add connection" : "Set up SimpleFin"}
        </button>
      </div>
      {sfStatus?.configured && (
        <div className="s-row">
          <div>
            <div className="label">Background sync</div>
            <div className="desc">Choose how often the FinSight server checks for updates.</div>
          </div>
          <div className="toolbar">
            {[0, 60, 180, 360, 720].map((minutes) => (
              <button
                key={minutes}
                className={(sfSyncSettings?.backgroundSyncIntervalMinutes ?? 360) === minutes ? "on" : ""}
                type="button"
                onClick={() => setSfSyncSettings.mutate({ backgroundSyncEnabled: minutes > 0, backgroundSyncIntervalMinutes: minutes })}
              >
                {minutes === 0 ? "Off" : minutes === 60 ? "1 hour" : minutes === 180 ? "3 hours" : minutes === 360 ? "6 hours" : "12 hours"}
              </button>
            ))}
          </div>
          <div />
        </div>
      )}
      {sfConnections.map((connection) => (
        <div key={connection.id} className="s-row">
          <div>
            <div className="label">{connection.label || connection.orgName || "SimpleFin connection"}</div>
            <div className="desc">
              {connection.status}
              {connection.lastSyncedAt ? ` · last synced ${new Date(connection.lastSyncedAt).toLocaleString()}` : ""}
            </div>
          </div>
          <div className="muted">Connected</div>
          <button
            className="btn ghost sm"
            type="button"
            onClick={() => deleteConnection.mutate(connection.id, { onSuccess: () => toast.success("Connection removed"), onError: () => toast.error("Failed to remove connection") })}
          >
            Remove
          </button>
        </div>
      ))}
      {sfConnections.length > 0 && (
        <div className="s-row">
          <div>
            <div className="label">Disconnect all</div>
            <div className="desc">Remove all stored SimpleFin credentials.</div>
          </div>
          <div />
          <button
            className="btn outline sm"
            type="button"
            onClick={() => disconnectSf.mutate(undefined, { onSuccess: () => toast.success("All SimpleFin credentials removed"), onError: () => toast.error("Failed to remove credentials") })}
          >
            Disconnect all
          </button>
        </div>
      )}
      {sfConnections.length > 0 && (
        <div className="s-row">
          <div>
            <div className="label">Remove imported SimpleFin data</div>
            <div className="desc">Deletes SimpleFin accounts, synced transactions, connection records, and stored credentials. Manual accounts are not touched.</div>
          </div>
          <div />
          <button
            className="btn outline sm"
            type="button"
            disabled={purgeSf.isPending}
            onClick={() => {
              if (!confirm("Remove all imported SimpleFin accounts and transactions from this FinSight profile? This keeps manual data but requires reconnecting SimpleFin.")) return;
              purgeSf.mutate(undefined, {
                onSuccess: (summary: { accountsDeleted: number; transactionsDeleted: number }) =>
                  toast.success("Imported SimpleFin data removed", {
                    description: `${summary.accountsDeleted} accounts and ${summary.transactionsDeleted} transactions removed.`,
                  }),
                onError: () => toast.error("Failed to remove imported SimpleFin data"),
              });
            }}
          >
            {purgeSf.isPending ? "Removing..." : "Remove imported data"}
          </button>
        </div>
      )}
      <SimpleFinDialog open={sfDialogOpen} onClose={() => setSfDialogOpen(false)} />
      {serverModeConn && <McpConnectionsSection />}
    </Section>
  );
}

export default function SettingsShortcuts() {
  const serverMode = isServerMode();
  const [serverAbout, setServerAbout] = useState<ServerAbout | null>(null);

  useEffect(() => {
    if (!serverMode) return;
    let cancelled = false;
    fetchServerAbout()
      .then((about) => {
        if (!cancelled) setServerAbout(about);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [serverMode]);

  return (
    <>
      <ConnectionsSection />
      <Section id="notifications" title="Notifications" description="Choose what you're notified about, when it stays quiet, and how much detail shows.">
        <NotificationPolicySettings />
        <PushNotificationSettings />
      </Section>

      <Section id="keyboard" title="Keyboard" description="Shortcuts available across the app.">
        <div className="s-row">
          <div>
            <div className="label">Command palette</div>
            <div className="desc">Jump to screens and quick actions.</div>
          </div>
          <div>
            <kbd className="tok">⌘K</kbd>
          </div>
          <div />
        </div>
        <div className="s-row">
          <div>
            <div className="label">Privacy mode</div>
            <div className="desc">Toggle amount blurring instantly.</div>
          </div>
          <div>
            <kbd className="tok">⌘.</kbd>
          </div>
          <div />
        </div>
      </Section>

      <Section id="about" title="About" description="Version and compatibility information.">
        <div className="s-row">
          <div>
            <div className="label">FinSight version</div>
            <div className="desc">The server version and web-client protocol used by this device.</div>
          </div>
          <div className="muted">
            {serverMode ? (serverAbout ? `Server ${serverAbout.version} · protocol ${serverAbout.protocol}` : "Server version unavailable") : `Web client · protocol ${CLIENT_PROTOCOL}`}
          </div>
          <div />
        </div>
      </Section>
    </>
  );
}
