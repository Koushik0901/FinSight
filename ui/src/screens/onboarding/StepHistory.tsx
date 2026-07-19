import { useMemo, useState } from "react";
import { useAccounts } from "../../api/hooks/accounts";
import { useTransactions } from "../../api/hooks/transactions";
import Button from "../../components/Button";
import Card from "../../components/Card";
import FilePicker from "../../components/FilePicker";
import ImportMappingDialog from "../../components/ImportMappingDialog";
import SimpleFinDialog from "./SimpleFinDialog";

interface Props {
  onBack: () => void;
  onNext: () => void;
}

interface CsvTarget {
  accountId: string;
  path: string;
}

export default function StepHistory({ onBack, onNext }: Props) {
  const [csvTarget, setCsvTarget] = useState<CsvTarget | null>(null);
  const [simpleFinOpen, setSimpleFinOpen] = useState(false);
  const { data: accounts = [], isLoading: accountsLoading, error: accountsError } = useAccounts();
  const { data: transactions = [], isLoading: transactionsLoading, error: transactionsError } = useTransactions();

  const transactionCounts = useMemo(() => {
    const counts = new Map<string, number>();
    for (const transaction of transactions) {
      counts.set(transaction.account_id, (counts.get(transaction.account_id) ?? 0) + 1);
    }
    return counts;
  }, [transactions]);

  const hasSimpleFin = accounts.some((account) => account.simplefin_account_id);
  const isLoading = accountsLoading || transactionsLoading;
  const hasError = !!accountsError || !!transactionsError;

  return (
    <div className="step-history onb-split">
      <div className="onb-left">
        <div className="num-step">003 · History</div>
        <h1>Bring in your history.</h1>
        <p className="lead">
          Import a CSV into a manual account. SimpleFIN accounts already receive their activity through sync.
        </p>

        <div className="row row-sm wrap onb-history-summary" aria-live="polite">
          <span className="chip">{accounts.length} account{accounts.length === 1 ? "" : "s"}</span>
          <span className={transactions.length > 0 ? "chip positive" : "chip"}>
            {transactions.length} transaction{transactions.length === 1 ? "" : "s"}
          </span>
        </div>

        {!hasSimpleFin && (
          <Card className="onb-inline-callout">
            <div>
              <strong>Prefer automatic bank sync?</strong>
              <p>SimpleFIN can discover accounts and keep their activity up to date.</p>
            </div>
            <Button variant="outline" size="sm" onClick={() => setSimpleFinOpen(true)}>
              Connect SimpleFIN
            </Button>
          </Card>
        )}

        {hasError && (
          <p role="alert" className="err">
            We could not read all local activity. You can continue and import later from Accounts.
          </p>
        )}

        <div className="onb-actions">
          <Button variant="primary" onClick={onNext} disabled={isLoading}>
            Continue to categories →
          </Button>
          <Button variant="ghost" onClick={onNext}>
            Do this later
          </Button>
        </div>
      </div>

      <div className="onb-right">
        <Card className="onb-roster-card">
          <div className="onb-roster-head">
            <div>
              <div className="eyebrow"><span className="dot" />Activity sources</div>
              <div className="h3">History by account</div>
            </div>
          </div>

          {isLoading && <p className="muted">Checking account activity…</p>}
          {!isLoading && accounts.length === 0 && (
            <div className="onb-empty-state">
              <div className="onb-empty-mark" aria-hidden="true">←</div>
              <strong>Add an account first</strong>
              <p>History needs an account destination. You can also skip and return later.</p>
              <Button variant="outline" size="sm" onClick={onBack}>
                Back to accounts
              </Button>
            </div>
          )}
          {accounts.length > 0 && (
            <div className="onb-account-list">
              {accounts.map((account) => {
                const count = transactionCounts.get(account.id) ?? 0;
                const isSimpleFin = !!account.simplefin_account_id;
                return (
                  <div className="onb-account-row onb-history-row" key={account.id}>
                    <span className="cswatch" style={{ background: account.color }} aria-hidden="true" />
                    <div className="onb-account-copy">
                      <strong>{account.nickname || account.name}</strong>
                      <span>
                        {isSimpleFin
                          ? "SimpleFIN sync" + (account.last_synced_at ? " · Synced" : " · Ready to sync")
                          : count + " transaction" + (count === 1 ? "" : "s")}
                      </span>
                    </div>
                    {isSimpleFin ? (
                      <span className="chip positive">Connected</span>
                    ) : (
                      <FilePicker
                        className="btn sm"
                        label={count > 0 ? "Import another CSV" : "Import CSV"}
                        onPicked={(path) => setCsvTarget({ accountId: account.id, path })}
                      />
                    )}
                  </div>
                );
              })}
            </div>
          )}
        </Card>
      </div>

      <SimpleFinDialog open={simpleFinOpen} onClose={() => setSimpleFinOpen(false)} />
      {csvTarget && (
        <ImportMappingDialog
          path={csvTarget.path}
          defaultAccountId={csvTarget.accountId}
          onClose={() => setCsvTarget(null)}
          onImported={() => setCsvTarget(null)}
        />
      )}
    </div>
  );
}
