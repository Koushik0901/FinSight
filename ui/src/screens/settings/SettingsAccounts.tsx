import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { toast } from "sonner";
import { fetchAuthStatus, isServerMode, logout, signOutOtherSessions } from "../../api/auth";
import { FINSIGHT_AUTH_REQUIRED } from "../../api/eventNames";
import { useResetOnboarding, useOnboardingState } from "../../api/hooks/onboarding";
import { Section } from "./Section";
import { userErrorMessage } from "../../utils/runtime";

export default function SettingsAccounts() {
  const navigate = useNavigate();
  const { data: onboarding } = useOnboardingState();
  const reset = useResetOnboarding();
  const serverMode = isServerMode();
  const [signingOut, setSigningOut] = useState(false);
  const [signingOutOthers, setSigningOutOthers] = useState(false);
  const [sessionUsername, setSessionUsername] = useState<string | null>(null);
  const [isAdmin, setIsAdmin] = useState(false);
  const [resetError, setResetError] = useState<string | null>(null);

  useEffect(() => {
    if (!serverMode) return;
    let cancelled = false;
    fetchAuthStatus()
      .then((status) => {
        if (!cancelled) {
          setIsAdmin(Boolean(status.isAdmin));
          setSessionUsername(status.username);
        }
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [serverMode]);

  const handleSignOut = async () => {
    setSigningOut(true);
    try {
      await logout();
    } catch (error) {
      toast.error("Sign out request failed", { description: userErrorMessage(error) });
    } finally {
      setSigningOut(false);
      window.dispatchEvent(new CustomEvent(FINSIGHT_AUTH_REQUIRED));
    }
  };

  const handleSignOutOthers = async () => {
    setSigningOutOthers(true);
    try {
      const count = await signOutOtherSessions();
      toast.success(
        count > 0
          ? `Signed out ${count} other ${count === 1 ? "device" : "devices"}`
          : "No other devices were signed in",
      );
    } catch (error) {
      toast.error("Couldn't sign out other devices", { description: userErrorMessage(error) });
    } finally {
      setSigningOutOthers(false);
    }
  };

  const reRunOnboarding = async () => {
    setResetError(null);
    try {
      await reset.mutateAsync();
      navigate("/onboarding?focus=accounts");
    } catch (error) {
      setResetError(userErrorMessage(error, "Could not reopen setup."));
    }
  };

  return (
    <>
      <Section id="profile" title="Profile" description="Who this setup is for and how to restart it.">
        <div className="s-row">
          <div>
            <div className="label">Onboarding</div>
            <div className="desc">Completed: {onboarding?.completion_marked ? "yes" : "no"}</div>
          </div>
          <div>{resetError && <div className="muted">{resetError}</div>}</div>
          <button className="btn sm" type="button" onClick={() => void reRunOnboarding()}>
            Re-run onboarding
          </button>
        </div>
        <div className="s-row">
          <div>
            <div className="label">FinSight profile</div>
            <div className="desc">
              {serverMode
                ? "Your encrypted data and preferences live on this FinSight server."
                : "Connect to a FinSight server to use a persistent profile."}
            </div>
          </div>
          <div className="muted">
            {serverMode ? (sessionUsername ? `Signed in as ${sessionUsername}` : "Server account") : "Not connected"}
          </div>
          <div />
        </div>
      </Section>

      {serverMode && (
        <Section id="account" title="Account" description="You're signed in on this FinSight server.">
          {isAdmin && (
            <div className="s-row">
              <div>
                <div className="label">Users</div>
                <div className="desc">Add or remove accounts on this server.</div>
              </div>
              <div />
              <button className="btn outline sm" type="button" onClick={() => navigate("/settings/users")}>
                Manage users
              </button>
            </div>
          )}
          <div className="s-row">
            <div>
              <div className="label">Sign out other devices</div>
              <div className="desc">
                Revoke every other signed-in session but keep this one. Use this if you&apos;ve lost a device or signed
                in somewhere you shouldn&apos;t have.
              </div>
            </div>
            <div />
            <button className="btn outline sm" type="button" disabled={signingOutOthers} onClick={() => void handleSignOutOthers()}>
              {signingOutOthers ? "Signing out…" : "Sign out other devices"}
            </button>
          </div>
          <div className="s-row">
            <div>
              <div className="label">Sign out</div>
              <div className="desc">End your session on this device. You&apos;ll need your password to sign back in.</div>
            </div>
            <div />
            <button className="btn outline sm" type="button" disabled={signingOut} onClick={() => void handleSignOut()}>
              {signingOut ? "Signing out…" : "Sign out"}
            </button>
          </div>
        </Section>
      )}
    </>
  );
}
