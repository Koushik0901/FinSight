import { useState, type FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";

/** First-run screen for the Phase 4 thin desktop shell: ask for the
 *  self-hosted FinSight server URL, verify it's reachable, store it in the
 *  OS keychain, then hand off to the caller (DesktopConnectGate) to navigate
 *  the window there. Only ever rendered inside the bundled shell app — never
 *  reachable once the window has navigated to a real server (that's a
 *  different origin serving the same ui/dist build, minus this screen's route
 *  ever being hit, since DesktopConnectGate only mounts pre-navigation). */
export default function ConnectScreen({ onConnected }: { onConnected: (url: string) => void }) {
  const [url, setUrl] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [checking, setChecking] = useState(false);

  async function handleConnect(e: FormEvent) {
    e.preventDefault();
    setError(null);
    const normalized = url.trim().replace(/\/+$/, "");
    if (!normalized) { setError("Enter your server's address."); return; }
    setChecking(true);
    try {
      const res = await fetch(`${normalized}/api/health`, { method: "GET" });
      if (!res.ok) throw new Error(`Server responded ${res.status}`);
      const body = await res.json();
      if (body.status !== "ok") throw new Error("Unexpected response from server");
      await invoke("set_server_url", { url: normalized });
      onConnected(normalized);
    } catch (err) {
      setError(
        err instanceof Error
          ? `Couldn't reach that server: ${err.message}`
          : "Couldn't reach that server."
      );
    } finally {
      setChecking(false);
    }
  }

  return (
    <div className="onb-stage" style={{ maxWidth: 440, margin: "80px auto" }}>
      <div className="card">
        <div className="eyebrow">Connect to your server</div>
        <h1 className="h1" style={{ marginTop: 8 }}>Where's your FinSight server?</h1>
        <p className="muted">
          Enter the address of your self-hosted FinSight server — for example a
          Tailscale hostname, a local network address, or a domain name.
        </p>
        <form onSubmit={(e) => void handleConnect(e)} style={{ marginTop: 20 }}>
          <input
            type="text"
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            placeholder="https://finsight.example.ts.net"
            autoFocus
            style={{ width: "100%" }}
          />
          {error && <p role="alert" style={{ color: "var(--negative)", marginTop: 8 }}>{error}</p>}
          <button className="btn primary" type="submit" disabled={checking} style={{ marginTop: 16 }}>
            {checking ? "Connecting…" : "Connect"}
          </button>
        </form>
      </div>
    </div>
  );
}
