import { useState } from "react";
import { useAccounts } from "../../api/hooks/accounts";
import { useTransactions } from "../../api/hooks/transactions";
import AccountDrawer from "../../components/AccountDrawer";
import TransactionDrawer from "../../components/TransactionDrawer";
import FilePicker from "../../components/FilePicker";
import ImportMappingDialog from "../../components/ImportMappingDialog";
import { isTauriRuntime } from "../../utils/runtime";
import Button from "../../components/Button";
import Card from "../../components/Card";
import SimpleFinDialog from "./SimpleFinDialog";

interface Props { onNext: () => void; }

export default function StepConnect({ onNext }: Props) {
  const [acctOpen, setAcctOpen] = useState(false);
  const [txnOpen, setTxnOpen]   = useState(false);
  const [csvPath, setCsvPath]   = useState<string | null>(null);
  const [sfOpen, setSfOpen]     = useState(false);

  const { data: accounts = [], isLoading: acctLoading, error: acctError } = useAccounts();
  const { data: txns = [], isLoading: txnLoading, error: txnError }       = useTransactions();

  const isLoading = acctLoading || txnLoading;
  const hasError  = !!acctError || !!txnError;

  const canContinue = accounts.length > 0;

  return (
    <div className="step-connect onb-split">
      <div className="onb-left">
        <div className="num-step">002 · Connect accounts</div>
        <h1>Connect your money source.</h1>
        <p className="lead">
          Import a CSV, add accounts manually, or connect with SimpleFin. You can combine methods and continue once at least one account is available.
        </p>
        {!hasError && isLoading && <p>Checking for accounts and recent transactions…</p>}
        {hasError && (
          <p role="alert" className="muted">
            We could not read your local data. You can still review setup options, then open the desktop app to import or save changes.
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
            <h3>Connect with SimpleFin</h3>
            <p>Link bank accounts securely using your SimpleFin bridge token.</p>
            <Button variant="default" onClick={() => setSfOpen(true)}>
              Set up SimpleFin
            </Button>
          </Card>

        </div>
      </div>

      <div className="onb-right">
        <Card className="stack stack-md">
          <div className="eyebrow"><span className="dot" />Setup progress</div>
          <div className="h3">Your data import status</div>
          <aside className="connect-tally" aria-live="polite">
            <strong>{accounts.length}</strong> account{accounts.length === 1 ? "" : "s"} added,{" "}
            <strong>{txns.length}</strong> transaction{txns.length === 1 ? "" : "s"} so far
          </aside>
          <div className="row row-sm wrap">
            <span className={`chip ${canContinue ? "positive" : ""}`}>
              {canContinue ? "Ready to continue" : "Add 1 account to continue"}
            </span>
            {txns.length > 0 && <span className="chip">Transactions detected</span>}
          </div>
          <Button variant="primary" disabled={!canContinue} onClick={onNext}>
            Continue →
          </Button>
          <Button variant="ghost" onClick={onNext}>
            Skip for now →
          </Button>
        </Card>
      </div>

      <AccountDrawer open={acctOpen} onClose={() => setAcctOpen(false)} />
      <TransactionDrawer open={txnOpen} onClose={() => setTxnOpen(false)} />
      <SimpleFinDialog open={sfOpen} onClose={() => setSfOpen(false)} />
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
