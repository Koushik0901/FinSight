import { useState } from "react";
import { toast } from "sonner";
import Drawer from "../../components/Drawer";
import Button from "../../components/Button";
import Card from "../../components/Card";
import {
  useSaveSimpleFinToken,
  useSimpleFinAccounts,
  useImportSimpleFinAccounts,
} from "../../api/hooks/simplefin";
import type { SimpleFinAccountInfo } from "../../api/bindings";

interface Props {
  open: boolean;
  onClose: () => void;
}

function accountKey(a: SimpleFinAccountInfo) {
  return `${a.connectionId}:${a.id}`;
}

export default function SimpleFinDialog({ open, onClose }: Props) {
  const [token, setToken] = useState("");
  const [step, setStep] = useState<"token" | "accounts">("token");
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [nicknames, setNicknames] = useState<Record<string, string>>({});

  const saveToken = useSaveSimpleFinToken();
  const { data: accounts, refetch: fetchAccounts, isFetching } = useSimpleFinAccounts();
  const importAccounts = useImportSimpleFinAccounts();

  const handleConnect = async () => {
    if (!token.trim()) return;
    try {
      await saveToken.mutateAsync(token.trim());
      toast.success("Connected to SimpleFin");
      const result = await fetchAccounts();
      if (result.data && result.data.length > 0) {
        setStep("accounts");
      } else {
        toast("No accounts found. Check your SimpleFin bridge setup.");
      }
    } catch (e) {
      toast.error("Failed to connect. The token may have expired or been used already.");
    }
  };

  const toggleAccount = (key: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(key)) {
        next.delete(key);
      } else {
        next.add(key);
      }
      return next;
    });
  };

  const handleImport = async () => {
    if (selected.size === 0 || !accounts) return;
    const byKey = new Map(accounts.map((a) => [accountKey(a), a]));
    const reqs = [...selected]
      .map((key) => {
        const a = byKey.get(key);
        if (!a) return null;
        return {
          simplefinId: a.id,
          connectionId: a.connectionId,
          nickname: nicknames[key] || null,
        };
      })
      .filter((r): r is NonNullable<typeof r> => r !== null);

    try {
      await importAccounts.mutateAsync(reqs);
      toast.success(`Imported ${reqs.length} account(s)`);
      onClose();
    } catch (e) {
      toast.error("Failed to import accounts");
    }
  };

  return (
    <Drawer open={open} onClose={onClose} title="Connect with SimpleFin">
      <div className="stack stack-lg" style={{ minHeight: 300 }}>
        {step === "token" && (
          <>
            <Card className="stack stack-md">
              <h3>Set up SimpleFin</h3>
              <p>
                SimpleFin lets you securely connect your bank accounts.
                Visit{" "}
                <a
                  href="https://bridge.simplefin.org/simplefin/create"
                  target="_blank"
                  rel="noopener noreferrer"
                >
                  bridge.simplefin.org
                </a>{" "}
                to generate a setup token, then paste it below.
              </p>
            </Card>

            <div className="stack stack-sm">
              <label htmlFor="sf-token">SimpleFin Token</label>
              <textarea
                id="sf-token"
                value={token}
                onChange={(e) => setToken(e.target.value)}
                rows={3}
                placeholder="Paste your SimpleFin token here…"
                style={{ fontFamily: "monospace", fontSize: "0.85rem" }}
              />
            </div>

            <footer>
              <Button
                variant="primary"
                onClick={handleConnect}
                disabled={!token.trim() || saveToken.isPending}
              >
                {saveToken.isPending ? "Connecting…" : "Connect"}
              </Button>
            </footer>
          </>
        )}

        {step === "accounts" && (
          <>
            <p>
              Select the accounts you want to import. You can also set an optional
              nickname for each one.
            </p>

            {isFetching && <p className="muted">Loading accounts…</p>}

            {accounts && accounts.length > 0 && (
              <div className="stack stack-md">
                {accounts.map((a) => {
                  const key = accountKey(a);
                  return (
                    <Card key={key} className="stack stack-sm">
                      <label style={{ display: "flex", alignItems: "center", gap: 8 }}>
                        <input
                          type="checkbox"
                          checked={selected.has(key)}
                          onChange={() => toggleAccount(key)}
                        />
                        <div style={{ flex: 1 }}>
                          <strong>{a.name}</strong>
                          <p className="muted">
                            {a.connectionName} · {a.accountType} · {a.currency} {a.balance}
                          </p>
                        </div>
                      </label>
                      {selected.has(key) && (
                        <input
                          type="text"
                          placeholder="Nickname (optional)"
                          value={nicknames[key] || ""}
                          onChange={(e) =>
                            setNicknames((prev) => ({
                              ...prev,
                              [key]: e.target.value,
                            }))
                          }
                          style={{ marginLeft: 28 }}
                        />
                      )}
                    </Card>
                  );
                })}
              </div>
            )}

            <footer style={{ display: "flex", gap: 8 }}>
              <Button variant="ghost" onClick={() => setStep("token")}>
                Back
              </Button>
              <Button
                variant="primary"
                onClick={handleImport}
                disabled={selected.size === 0 || importAccounts.isPending}
              >
                {importAccounts.isPending
                  ? "Importing…"
                  : `Import ${selected.size} account${selected.size === 1 ? "" : "s"}`}
              </Button>
            </footer>
          </>
        )}
      </div>
    </Drawer>
  );
}
