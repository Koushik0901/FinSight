import { useState } from "react";
import { useAccounts } from "../../api/hooks/accounts";
import { useTransactions } from "../../api/hooks/transactions";
import AccountDrawer from "../../components/AccountDrawer";
import TransactionDrawer from "../../components/TransactionDrawer";
import FilePicker from "../../components/FilePicker";
import ImportMappingDialog from "./ImportMappingDialog";
import { isTauriRuntime } from "../../utils/runtime";
import Button from "../../components/Button";
import Card from "../../components/Card";

interface Props { onNext: () => void; }

export default function StepConnect({ onNext }: Props) {
  const [acctOpen, setAcctOpen] = useState(false);
  const [txnOpen, setTxnOpen]   = useState(false);
  const [csvPath, setCsvPath]   = useState<string | null>(null);

  const { data: accounts = [], isLoading: acctLoading, error: acctError } = useAccounts();
  const { data: txns = [], isLoading: txnLoading, error: txnError }       = useTransactions();

  const isLoading = acctLoading || txnLoading;
  const hasError  = !!acctError || !!txnError;

  const canContinue = accounts.length > 0;

  return (
    <div className="step-connect">
      <h2>Connect your money</h2>

      {!hasError && isLoading && <p>Checking for accounts and recent transactions…</p>}
      {hasError && (
        <p role="alert" className="muted">
          We could not read your local data. You can still review the setup options, then open the desktop app to import or save changes.
        </p>
      )}
      {!isTauriRuntime() && (
        <p className="muted">
          Browser preview mode: imports and manual saves require the desktop app runtime.
        </p>
      )}

      <div className="connect-cards">
        <Card className="stack stack-md">
          <h3>Import a statement</h3>
          <p>Pick a CSV exported from your bank and map its columns.</p>
          <FilePicker onPicked={setCsvPath} label="Pick a file…" />
        </Card>

        <Card className="stack stack-md">
          <h3>Add manually</h3>
          <p>Walk through accounts and a few recent transactions by hand.</p>
          <div className="button-row">
            <Button variant="default" onClick={() => setAcctOpen(true)}>+ Account</Button>
            <Button variant="default" onClick={() => setTxnOpen(true)} disabled={accounts.length === 0}>+ Transaction</Button>
          </div>
        </Card>

        <Card className="stack stack-md">
          <h3>Skip for now</h3>
          <p>You can always add or import later from the Accounts screen.</p>
          <Button variant="ghost" onClick={onNext}>Skip →</Button>
        </Card>
      </div>

      {(!isLoading || hasError) && (
        <>
          <aside className="connect-tally" aria-live="polite">
            <strong>{accounts.length}</strong> account{accounts.length === 1 ? "" : "s"} added,{" "}
            <strong>{txns.length}</strong> transaction{txns.length === 1 ? "" : "s"} so far
          </aside>

          <footer>
            <Button variant="primary" disabled={!canContinue} onClick={onNext}>
              Continue →
            </Button>
          </footer>
        </>
      )}

      <AccountDrawer open={acctOpen} onClose={() => setAcctOpen(false)} />
      <TransactionDrawer open={txnOpen} onClose={() => setTxnOpen(false)} />
      {csvPath && (
        <ImportMappingDialog
          path={csvPath}
          onClose={() => setCsvPath(null)}
          onImported={() => setCsvPath(null)}
        />
      )}
    </div>
  );
}
