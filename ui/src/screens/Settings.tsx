import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { useAccounts } from "../api/hooks/accounts";
import {
  useResetOnboarding,
  useClearSampleData,
  useOnboardingState,
} from "../api/hooks/onboarding";

export default function Settings() {
  const navigate = useNavigate();
  const reset = useResetOnboarding();
  const clearSample = useClearSampleData();
  const { data: accounts = [] } = useAccounts();
  const { data: onboarding } = useOnboardingState();
  const hasSample = accounts.some((a) => a.source === "sample");
  const [resetError, setResetError] = useState<string | null>(null);
  const [clearError, setClearError] = useState<string | null>(null);

  async function reRunOnboarding() {
    if (
      !confirm(
        "This will re-open the welcome wizard. Your existing accounts, transactions, and categories are kept."
      )
    )
      return;
    setResetError(null);
    try {
      await reset.mutateAsync();
      navigate("/onboarding");
    } catch (err) {
      setResetError(err instanceof Error ? err.message : "Something went wrong.");
    }
  }

  async function replaceSampleData() {
    if (
      !confirm(
        "This will permanently delete the Mira & Adam sample accounts and their transactions. Anything you added manually or imported is kept."
      )
    )
      return;
    setClearError(null);
    try {
      await clearSample.mutateAsync();
      navigate("/onboarding");
    } catch (err) {
      setClearError(err instanceof Error ? err.message : "Something went wrong.");
    }
  }

  return (
    <div className="screen-settings">
      <h1 style={{ fontSize: 32, fontWeight: 600, marginTop: 0, marginBottom: 24 }}>Settings</h1>

      <section style={{ marginBottom: 32 }}>
        <h2 style={{ fontSize: 18, fontWeight: 600, marginBottom: 8 }}>Onboarding</h2>
        <p style={{ marginBottom: 12 }}>
          Completed: <strong>{onboarding?.completion_marked ? "yes" : "no"}</strong>
        </p>
        {resetError && (
          <p role="alert" style={{ color: "var(--error, red)", marginBottom: 8 }}>
            {resetError}
          </p>
        )}
        <button onClick={reRunOnboarding}>Re-run onboarding</button>
      </section>

      {hasSample && (
        <section>
          <h2 style={{ fontSize: 18, fontWeight: 600, marginBottom: 8 }}>Sample data</h2>
          <p style={{ marginBottom: 12 }}>
            You&apos;re currently looking at the Mira &amp; Adam sample household. Replace it with
            your own when you&apos;re ready.
          </p>
          {clearError && (
            <p role="alert" style={{ color: "var(--error, red)", marginBottom: 8 }}>
              {clearError}
            </p>
          )}
          <button onClick={replaceSampleData} className="danger">
            Replace sample data with my own
          </button>
        </section>
      )}
    </div>
  );
}
