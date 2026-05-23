import { useState } from "react";
import { useAccounts } from "../../api/hooks/accounts";
import { useTransactions } from "../../api/hooks/transactions";
import AccountDrawer from "../../components/AccountDrawer";
import TransactionDrawer from "../../components/TransactionDrawer";

interface Props { onNext: () => void; }

export default function StepConnect({ onNext }: Props) {
  const [acctOpen, setAcctOpen] = useState(false);
  const [txnOpen, setTxnOpen]   = useState(false);

  const { data: accounts = [], isLoading: acctLoading, error: acctError } = useAccounts();
  const { data: txns = [], isLoading: txnLoading, error: txnError }       = useTransactions();

  const isLoading = acctLoading || txnLoading;
  const hasError  = !!acctError || !!txnError;

  const canContinue = accounts.length > 0;

  return (
    <div className="step-connect">
      <h2>Connect your money</h2>

      {isLoading && <p>Loading…</p>}
      {hasError && (
        <p role="alert" style={{ color: "var(--error, red)" }}>
          Failed to load data. Please try again.
        </p>
      )}

      <div className="connect-cards">
        <article className="card">
          <h3>Import a statement</h3>
          <p>Pick a CSV exported from your bank and map its columns.</p>
          <button disabled title="Filled in Task 19">Pick a file…</button>
        </article>

        <article className="card">
          <h3>Add manually</h3>
          <p>Walk through accounts and a few recent transactions by hand.</p>
          <div className="button-row">
            <button onClick={() => setAcctOpen(true)}>+ Account</button>
            <button onClick={() => setTxnOpen(true)} disabled={accounts.length === 0}>+ Transaction</button>
          </div>
        </article>

        <article className="card">
          <h3>Skip for now</h3>
          <p>You can always add or import later from the Accounts screen.</p>
          <button onClick={onNext}>Skip →</button>
        </article>
      </div>

      {!isLoading && (
        <>
          <aside className="connect-tally" aria-live="polite">
            <strong>{accounts.length}</strong> account{accounts.length === 1 ? "" : "s"} added,{" "}
            <strong>{txns.length}</strong> transaction{txns.length === 1 ? "" : "s"} so far
          </aside>

          <footer>
            <button className="primary" disabled={!canContinue} onClick={onNext}>
              Continue →
            </button>
          </footer>
        </>
      )}

      <AccountDrawer open={acctOpen} onClose={() => setAcctOpen(false)} />
      <TransactionDrawer open={txnOpen} onClose={() => setTxnOpen(false)} />
    </div>
  );
}
