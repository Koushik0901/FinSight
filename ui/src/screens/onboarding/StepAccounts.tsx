import { useState } from "react";
import { useAccounts } from "../../api/hooks/accounts";
import AccountDrawer from "../../components/AccountDrawer";
import Button from "../../components/Button";
import Card from "../../components/Card";
import { isBackendAvailable } from "../../utils/runtime";
import SimpleFinDialog from "./SimpleFinDialog";

interface Props {
  onNext: () => void;
}

export default function StepAccounts({ onNext }: Props) {
  const [accountOpen, setAccountOpen] = useState(false);
  const [simpleFinOpen, setSimpleFinOpen] = useState(false);
  const { data: accounts = [], isLoading, error } = useAccounts();

  return (
    <div className="step-accounts onb-split">
      <div className="onb-left">
        <div className="num-step">002 · Accounts</div>
        <h1>Start with your accounts.</h1>
        <p className="lead">
          Add the places where you keep, spend, invest, or borrow money. Transactions come next.
        </p>

        <div className="onb-choice-grid">
          <Card className="onb-choice-card">
            <div className="eyebrow">Manual</div>
            <h3>Create an account</h3>
            <p>Add a checking, savings, credit, investment, cash, or loan account yourself.</p>
            <Button variant="primary" onClick={() => setAccountOpen(true)}>
              + Add account
            </Button>
          </Card>
          <Card className="onb-choice-card">
            <div className="eyebrow">Automatic</div>
            <h3>Discover with SimpleFIN</h3>
            <p>Connect once, choose your bank accounts, and let FinSight create them for you.</p>
            <Button variant="outline" onClick={() => setSimpleFinOpen(true)}>
              Connect SimpleFIN
            </Button>
          </Card>
        </div>

        {!isBackendAvailable() && (
          <p className="muted onb-runtime-note">
            Browser preview mode: creating or connecting accounts requires the desktop app.
          </p>
        )}
        {error && (
          <p role="alert" className="err">
            We could not read your accounts. You can still continue and finish setup later.
          </p>
        )}

        <div className="onb-actions">
          <Button
            variant={accounts.length > 0 ? "primary" : "outline"}
            onClick={onNext}
            disabled={isLoading}
          >
            {accounts.length > 0 ? "Continue to history →" : "I’ll add accounts later →"}
          </Button>
        </div>
      </div>

      <div className="onb-right">
        <Card className="onb-roster-card">
          <div className="onb-roster-head">
            <div>
              <div className="eyebrow"><span className="dot" />Your account roster</div>
              <div className="h3">What FinSight will track</div>
            </div>
            <span className="chip" aria-live="polite">
              {accounts.length} account{accounts.length === 1 ? "" : "s"}
            </span>
          </div>

          {isLoading && <p className="muted">Checking your accounts…</p>}
          {!isLoading && accounts.length === 0 && (
            <div className="onb-empty-state">
              <div className="onb-empty-mark" aria-hidden="true">+</div>
              <strong>No accounts yet</strong>
              <p>Add one manually or let SimpleFIN discover your connected accounts.</p>
            </div>
          )}
          {accounts.length > 0 && (
            <div className="onb-account-list">
              {accounts.map((account) => (
                <div className="onb-account-row" key={account.id}>
                  <span className="cswatch" style={{ background: account.color }} aria-hidden="true" />
                  <div className="onb-account-copy">
                    <strong>{account.nickname || account.name}</strong>
                    <span>
                      {account.bank} · {account.type}
                      {account.mask ? " · •••• " + account.mask : ""}
                    </span>
                  </div>
                  <span className={account.simplefin_account_id ? "chip positive" : "chip"}>
                    {account.simplefin_account_id ? "SimpleFIN" : "Manual"}
                  </span>
                </div>
              ))}
            </div>
          )}
        </Card>
      </div>

      <AccountDrawer open={accountOpen} onClose={() => setAccountOpen(false)} />
      <SimpleFinDialog open={simpleFinOpen} onClose={() => setSimpleFinOpen(false)} />
    </div>
  );
}
