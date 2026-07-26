import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";
import Button from "./Button";
import Input from "./Input";
import {
  createApiToken,
  listApiTokens,
  mcpEndpointUrl,
  revokeApiToken,
  type ApiTokenSummary,
  type CreatedApiToken,
  type TokenScope,
} from "../api/tokens";
import { userErrorMessage } from "../utils/runtime";

/**
 * Server-mode-only: connect an external assistant (Claude, ChatGPT) to this
 * FinSight server over MCP, so the user's own subscription drives the same
 * tools the in-app Copilot uses.
 *
 * Rendered inside Settings → Connections. Uses plain `useState`/`useEffect`
 * rather than react-query because these endpoints are REST, not RPC commands
 * (same shape as UsersAdmin).
 */
export default function McpConnectionsSection() {
  const [tokens, setTokens] = useState<ApiTokenSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);

  const [creating, setCreating] = useState(false);
  const [name, setName] = useState("");
  const [scope, setScope] = useState<TokenScope>("read");
  const [submitting, setSubmitting] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);
  // Held only for this render — never persisted, never re-fetchable.
  const [issued, setIssued] = useState<CreatedApiToken | null>(null);
  const [revokingId, setRevokingId] = useState<string | null>(null);

  const endpoint = mcpEndpointUrl();

  const refresh = useCallback(async () => {
    setLoading(true);
    setLoadError(null);
    try {
      setTokens(await listApiTokens());
    } catch (err) {
      setLoadError(userErrorMessage(err, "Could not load access tokens."));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const copy = async (text: string, label: string) => {
    try {
      await navigator.clipboard.writeText(text);
      toast.success(`${label} copied`);
    } catch {
      toast.error(`Could not copy — select and copy the ${label.toLowerCase()} manually`);
    }
  };

  const handleCreate = async () => {
    setFormError(null);
    if (!name.trim()) {
      setFormError("Give this token a name so you can recognise it later.");
      return;
    }
    setSubmitting(true);
    try {
      const created = await createApiToken(name.trim(), scope);
      setIssued(created);
      setName("");
      setScope("read");
      setCreating(false);
      await refresh();
    } catch (err) {
      const message = userErrorMessage(err, "Could not create the token.");
      setFormError(message);
      toast.error("Create token failed", { description: message });
    } finally {
      setSubmitting(false);
    }
  };

  const handleRevoke = async (token: ApiTokenSummary) => {
    if (
      !window.confirm(
        `Revoke "${token.name}"? Any assistant using this token loses access to your data immediately.`
      )
    )
      return;
    setRevokingId(token.id);
    try {
      await revokeApiToken(token.id);
      toast.success(`Revoked ${token.name}`);
      await refresh();
    } catch (err) {
      toast.error("Revoke failed", { description: userErrorMessage(err, "Could not revoke the token.") });
    } finally {
      setRevokingId(null);
    }
  };

  return (
    <>
      <div className="s-row">
        <div>
          <div className="label">MCP server address</div>
          <div className="desc">
            Point Claude or ChatGPT here to use your own AI subscription instead of the in-app Copilot. They
            get the same tools, reading this server&apos;s data.
          </div>
        </div>
        <div className="muted num" style={{ wordBreak: "break-all" }}>
          {endpoint}
        </div>
        <Button type="button" variant="outline" size="sm" onClick={() => void copy(endpoint, "Address")}>
          Copy
        </Button>
      </div>

      {issued && (
        <div
          className="card"
          style={{ marginTop: 12, borderColor: "var(--accent)" }}
          data-testid="issued-token"
        >
          <p className="eyebrow">Copy your token now</p>
          <p className="muted" style={{ marginTop: 6, fontSize: 13.5, lineHeight: 1.55 }}>
            This is the only time it is shown. It grants{" "}
            {issued.scope === "full" ? "read and write" : "read-only"} access to your financial data — treat
            it like a password. If you lose it, revoke it and create another.
          </p>
          <div
            style={{
              marginTop: 12,
              padding: "12px 14px",
              background: "var(--surface-2)",
              border: "1px solid var(--line)",
              borderRadius: "var(--radius, 10px)",
              fontFamily: "var(--mono)",
              fontSize: 13,
              wordBreak: "break-all",
            }}
          >
            {issued.token}
          </div>
          <div className="toolbar" style={{ marginTop: 12 }}>
            <Button type="button" variant="outline" size="sm" onClick={() => void copy(issued.token, "Token")}>
              Copy token
            </Button>
            <Button type="button" variant="primary" size="sm" onClick={() => setIssued(null)}>
              Done
            </Button>
          </div>

          <p className="muted" style={{ marginTop: 16, fontSize: 12.5 }}>
            In Claude Code, connect with:
          </p>
          <div
            style={{
              marginTop: 6,
              padding: "10px 12px",
              background: "var(--surface-2)",
              border: "1px solid var(--line)",
              borderRadius: "var(--radius, 10px)",
              fontFamily: "var(--mono)",
              fontSize: 12,
              wordBreak: "break-all",
            }}
          >
            {`claude mcp add --transport http finsight ${endpoint} --header "Authorization: Bearer ${issued.token}"`}
          </div>
        </div>
      )}

      <div className="s-row">
        <div>
          <div className="label">Access tokens</div>
          <div className="desc">
            Each connected assistant gets its own token. Revoking one cuts off that assistant only. Resetting
            your password with a recovery key revokes all of them.
          </div>
        </div>
        <div className="muted">
          {loading ? "Loading…" : `${tokens.length} token${tokens.length === 1 ? "" : "s"}`}
        </div>
        <Button type="button" variant="outline" size="sm" onClick={() => setCreating((v) => !v)}>
          {creating ? "Cancel" : "Create token"}
        </Button>
      </div>

      {loadError && (
        <p role="alert" className="err" style={{ marginTop: 8 }}>
          {loadError}
        </p>
      )}

      {creating && (
        <div className="card" style={{ marginTop: 12 }}>
          <div style={{ display: "flex", flexDirection: "column", gap: 12, maxWidth: 420 }}>
            <Input
              label="Name"
              id="mcp-token-name"
              value={name}
              placeholder="Claude Desktop"
              autoComplete="off"
              onChange={(e) => setName(e.target.value)}
            />
            <div>
              <div className="label" id="mcp-token-scope-label">
                Access level
              </div>
              <div className="toolbar" style={{ marginTop: 6 }} role="group" aria-labelledby="mcp-token-scope-label">
                <button
                  type="button"
                  className={scope === "read" ? "on" : ""}
                  aria-pressed={scope === "read"}
                  onClick={() => setScope("read")}
                >
                  Read only
                </button>
                <button
                  type="button"
                  className={scope === "full" ? "on" : ""}
                  aria-pressed={scope === "full"}
                  onClick={() => setScope("full")}
                >
                  Read and write
                </button>
              </div>
              <div className="desc" style={{ marginTop: 6 }}>
                {scope === "read"
                  ? "The assistant can analyse your finances but cannot change anything."
                  : "The assistant can also propose changes and, once you agree in the conversation, apply them. Proposals still appear in FinSight for review."}
              </div>
            </div>
          </div>

          {formError && (
            <p role="alert" className="err" style={{ marginTop: 12 }}>
              {formError}
            </p>
          )}

          <Button
            type="button"
            variant="primary"
            size="sm"
            style={{ marginTop: 14 }}
            disabled={submitting}
            onClick={() => void handleCreate()}
          >
            {submitting ? "Creating…" : "Create token"}
          </Button>
        </div>
      )}

      {!loading && tokens.length > 0 && (
        <div className="tbl-scroll" style={{ marginTop: 12 }}>
          <table className="tbl">
            <thead>
              <tr>
                <th>Name</th>
                <th>Access</th>
                <th>Created</th>
                <th>Last used</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {tokens.map((t) => (
                <tr key={t.id}>
                  <td>{t.name}</td>
                  <td>
                    <span className={t.scope === "full" ? "chip accent" : "chip"}>
                      {t.scope === "full" ? "Read and write" : "Read only"}
                    </span>
                  </td>
                  <td className="muted">{new Date(t.createdAt).toLocaleDateString()}</td>
                  <td className="muted">
                    {t.lastUsedAt ? new Date(t.lastUsedAt).toLocaleString() : "Never"}
                  </td>
                  <td className="right">
                    <Button
                      type="button"
                      variant="danger"
                      size="sm"
                      disabled={revokingId === t.id}
                      onClick={() => void handleRevoke(t)}
                    >
                      {revokingId === t.id ? "Revoking…" : "Revoke"}
                    </Button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </>
  );
}
