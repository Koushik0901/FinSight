import { useCallback, useEffect, useState, type FormEvent } from "react";
import { toast } from "sonner";
import Button from "../../components/Button";
import Input from "../../components/Input";
import { RecoveryKeyReveal } from "../../components/RecoveryKeyReveal";
import { createUser, deleteUser, fetchAuthStatus, isServerMode, listUsers, type AdminUser } from "../../api/auth";
import { userErrorMessage } from "../../utils/runtime";

/**
 * Server-mode-only admin surface: list users, add a new user (revealing the
 * one-time recovery key via the shared RecoveryKeyReveal component), and
 * delete users other than the currently-signed-in one.
 *
 * Renders nothing outside server mode, and nothing for a non-admin — the
 * backend also 403s (`auth.admin_required`) for non-admin callers, but we
 * gate visibility client-side too so the surface never even attempts the
 * calls for a non-admin session.
 */
export default function UsersAdmin() {
  const serverMode = isServerMode();
  const [currentUsername, setCurrentUsername] = useState<string | null>(null);
  const [isAdmin, setIsAdmin] = useState<boolean | null>(null);
  const [users, setUsers] = useState<AdminUser[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);

  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [formError, setFormError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [recoveryKey, setRecoveryKey] = useState<string | null>(null);
  const [deletingId, setDeletingId] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setLoadError(null);
    try {
      // Resolve admin status first: the backend 403s `GET /api/auth/users`
      // for non-admins (`auth.admin_required`), so listUsers() must not be
      // called until we know isAdmin is true — otherwise a non-admin would
      // see a load error instead of the "not authorized" note below.
      const status = await fetchAuthStatus();
      setCurrentUsername(status.username);
      setIsAdmin(status.isAdmin ?? false);
      if (!status.isAdmin) return;
      setUsers(await listUsers());
    } catch (err) {
      setLoadError(userErrorMessage(err, "Could not load users."));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (!serverMode) return;
    void refresh();
  }, [serverMode, refresh]);

  if (!serverMode) return null;

  const handleAddUser = async (e: FormEvent) => {
    e.preventDefault();
    setFormError(null);

    if (!username.trim() || !password) {
      setFormError("Enter a username and password.");
      return;
    }

    setSubmitting(true);
    try {
      const result = await createUser(username.trim(), password);
      setRecoveryKey(result.recoveryKey);
      setUsername("");
      setPassword("");
    } catch (err) {
      const code = (err as { code?: string } | null)?.code;
      const message =
        code === "auth.username_taken"
          ? "That username is already taken."
          : userErrorMessage(err, "Could not create user.");
      setFormError(message);
      toast.error("Add user failed", { description: message });
    } finally {
      setSubmitting(false);
    }
  };

  const handleRecoveryKeyContinue = () => {
    setRecoveryKey(null);
    void refresh();
  };

  const handleDelete = async (user: AdminUser) => {
    if (!window.confirm(`Delete "${user.username}"? This removes their account and all of their data.`)) return;
    setDeletingId(user.id);
    try {
      await deleteUser(user.id);
      toast.success(`Deleted ${user.username}`);
      await refresh();
    } catch (err) {
      const code = (err as { code?: string } | null)?.code;
      const message =
        code === "auth.cannot_delete_self"
          ? "You can't delete your own account."
          : userErrorMessage(err, "Could not delete user.");
      toast.error("Delete failed", { description: message });
    } finally {
      setDeletingId(null);
    }
  };

  if (isAdmin === false) {
    return (
      <div className="screen users-admin-screen">
        <div className="card">
          <p className="eyebrow">Users</p>
          <p className="muted" style={{ marginTop: 8 }}>You don&apos;t have permission to manage users on this server.</p>
        </div>
      </div>
    );
  }

  if (recoveryKey) {
    return (
      <div className="screen users-admin-screen">
        <RecoveryKeyReveal recoveryKey={recoveryKey} onContinue={handleRecoveryKeyContinue} />
      </div>
    );
  }

  return (
    <div className="screen users-admin-screen">
      <div className="card">
        <p className="eyebrow">Server administration</p>
        <h1 className="h1" style={{ fontSize: 22 }}>Users</h1>
        <p className="muted" style={{ marginTop: 8 }}>
          Manage who can sign in to this FinSight server. Each user has their own separate financial data.
        </p>

        {loading && <p className="muted" style={{ marginTop: 16 }}>Loading…</p>}
        {loadError && (
          <p role="alert" className="err" style={{ marginTop: 16 }}>
            {loadError}
          </p>
        )}

        {!loading && !loadError && (
          <table className="tbl" style={{ marginTop: 16 }}>
            <thead>
              <tr>
                <th>Username</th>
                <th>Created</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {users.map((u) => {
                const isSelf = u.username === currentUsername;
                return (
                  <tr key={u.id}>
                    <td>
                      {u.username}
                      {u.isAdmin && (
                        <span className="chip accent" style={{ marginLeft: 8 }}>
                          Admin
                        </span>
                      )}
                    </td>
                    <td className="muted">{new Date(u.createdAt).toLocaleDateString()}</td>
                    <td className="right">
                      <Button
                        type="button"
                        variant="danger"
                        size="sm"
                        disabled={isSelf || deletingId === u.id}
                        title={isSelf ? "You can't delete your own account" : undefined}
                        onClick={() => void handleDelete(u)}
                      >
                        {deletingId === u.id ? "Deleting…" : "Delete"}
                      </Button>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        )}
      </div>

      <div className="card" style={{ marginTop: 24 }}>
        <p className="eyebrow">Add user</p>
        <form onSubmit={(e) => void handleAddUser(e)}>
          <div style={{ marginTop: 12, display: "flex", flexDirection: "column", gap: 12, maxWidth: 360 }}>
            <Input
              label="Username"
              id="admin-add-username"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              autoComplete="off"
            />
            <Input
              label="Password"
              id="admin-add-password"
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              autoComplete="new-password"
            />
          </div>

          {formError && (
            <p role="alert" className="err" style={{ marginTop: 12 }}>
              {formError}
            </p>
          )}

          <Button type="submit" variant="primary" style={{ marginTop: 16 }} disabled={submitting}>
            {submitting ? "Adding…" : "Add user"}
          </Button>
        </form>
      </div>
    </div>
  );
}
