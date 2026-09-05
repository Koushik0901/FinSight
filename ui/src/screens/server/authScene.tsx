/**
 * Server-mode auth scene — shared shell, fields, and a restrained trust brief.
 * SetupScreen / LoginScreen / RecoverScreen render their form bodies inside
 * {@link AuthShell}. All styling is scoped under `.fs-auth` (see auth.css) so the
 * auth surface stays isolated from the signed-in application shell.
 *
 * The right pane explains FinSight's privacy posture without fabricated data.
 */
import {
  useState,
  type ReactNode,
  type SVGProps,
} from "react";
import { toast } from "sonner";
import "../../styles/auth.css";

/* ── icons ─────────────────────────────────────────────── */
type IcoProps = SVGProps<SVGSVGElement>;
export const Ico = {
  user: (p: IcoProps = {}) => (
    <svg viewBox="0 0 16 16" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" {...p}><circle cx="8" cy="5.5" r="2.6" /><path d="M3 13.2a5 5 0 0 1 10 0" /></svg>
  ),
  lock: (p: IcoProps = {}) => (
    <svg viewBox="0 0 16 16" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" {...p}><rect x="3" y="7" width="10" height="7" rx="1.6" /><path d="M5 7V5a3 3 0 0 1 6 0v2" /></svg>
  ),
  key: (p: IcoProps = {}) => (
    <svg viewBox="0 0 16 16" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" {...p}><circle cx="5.5" cy="5.5" r="3" /><path d="m7.6 7.6 5 5M11 11l1.4-1.4M9.6 9.6 11 8.2" /></svg>
  ),
  eye: (p: IcoProps = {}) => (
    <svg viewBox="0 0 16 16" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" {...p}><path d="M1.5 8s2.5-4.5 6.5-4.5S14.5 8 14.5 8 12 12.5 8 12.5 1.5 8 1.5 8z" /><circle cx="8" cy="8" r="1.8" /></svg>
  ),
  eyeoff: (p: IcoProps = {}) => (
    <svg viewBox="0 0 16 16" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" {...p}><path d="M3 3l10 10" /><path d="M6 6.2C3.8 7.4 1.5 8 1.5 8s2.5 4.5 6.5 4.5c1.1 0 2.1-.3 3-.7" /><path d="M9.6 4.1A6.7 6.7 0 0 1 14.5 8s-.7 1.3-2 2.5" /></svg>
  ),
  check: (p: IcoProps = {}) => (
    <svg viewBox="0 0 16 16" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" {...p}><path d="m3 8 3.5 3.5L13 5" /></svg>
  ),
  arrow: (p: IcoProps = {}) => (
    <svg viewBox="0 0 16 16" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" {...p}><path d="M3 8h9M8.5 4l4 4-4 4" /></svg>
  ),
  warn: (p: IcoProps = {}) => (
    <svg viewBox="0 0 16 16" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" {...p}><path d="M8 5.5v3.5M8 11.2v.1" /><circle cx="8" cy="8" r="6.2" /></svg>
  ),
};

/* ── floating-label field ──────────────────────────────── */
export function Field({
  icon,
  label,
  type = "text",
  value,
  onChange,
  autoComplete,
  error,
  valid,
  required,
  autoFocus,
  id,
  trailing,
}: {
  icon: ReactNode;
  label: string;
  type?: string;
  value: string;
  onChange: (v: string) => void;
  autoComplete?: string;
  error?: string | null;
  valid?: boolean;
  required?: boolean;
  autoFocus?: boolean;
  id?: string;
  trailing?: ReactNode;
}) {
  const [focus, setFocus] = useState(false);
  const filled = value.length > 0;
  const cls = ["field", focus && "focused", filled && "filled", error && "invalid", valid && !error && "valid"]
    .filter(Boolean)
    .join(" ");
  return (
    <div className={cls}>
      <label className="lab" htmlFor={id}>{label}{required && <b> *</b>}</label>
      <div className="in-wrap">
        <div className="lead-ico">{icon}</div>
        <input
          id={id}
          type={type}
          value={value}
          autoComplete={autoComplete}
          autoFocus={autoFocus}
          placeholder={label}
          aria-label={label}
          aria-invalid={!!error}
          onChange={(e) => onChange(e.target.value)}
          onFocus={() => setFocus(true)}
          onBlur={() => setFocus(false)}
        />
        <div className="trail">
          {valid && !error && <span className="valid-check">{Ico.check()}</span>}
          {trailing}
        </div>
      </div>
      {error && <div className="field-err">{Ico.warn()} {error}</div>}
    </div>
  );
}

/* ── password strength ─────────────────────────────────── */
export function strength(pw: string): number {
  let s = 0;
  if (pw.length >= 10) s++;
  if (/[A-Z]/.test(pw) && /[a-z]/.test(pw)) s++;
  if (/\d/.test(pw)) s++;
  if (/[^A-Za-z0-9]/.test(pw)) s++;
  return Math.min(s, 4);
}
export const STR = [
  { lab: "TOO SHORT", c: "var(--ink-faint)", hint: "10+ characters" },
  { lab: "WEAK", c: "var(--negative)", hint: "Add a number" },
  { lab: "FAIR", c: "var(--warning)", hint: "Add a symbol" },
  { lab: "GOOD", c: "var(--sky)", hint: "Nearly there" },
  { lab: "STRONG", c: "var(--positive)", hint: "Great password" },
];

export function PasswordStrength({ pw, open }: { pw: string; open: boolean }) {
  const st = strength(pw);
  const meta = STR[st]!;
  return (
    <div className={"collapse" + (open ? " open" : "")}>
      <div className="collapse-inner">
        <div className="pw-strength">
          <div className="pw-bars">
            {[0, 1, 2, 3].map((i) => (
              <span key={i} style={{ background: i < st ? meta.c : undefined }} />
            ))}
          </div>
          <div className="pw-meta">
            <span className="lvl" style={{ color: meta.c }}>{meta.lab}</span>
            <span className="hint">{meta.hint}</span>
          </div>
        </div>
      </div>
    </div>
  );
}

const SHOWCASE_POINTS = [
  {
    label: "Private by default",
    detail: "Your ledger stays on the FinSight server you control.",
  },
  {
    label: "Encrypted at rest",
    detail: "Each profile uses its own protected financial database.",
  },
  {
    label: "Useful without surveillance",
    detail: "No advertising profile, engagement loop, or data brokerage.",
  },
] as const;

const DECISION_STEPS = [
  { label: "See", detail: "Find the signal" },
  { label: "Understand", detail: "Know what changed" },
  { label: "Plan", detail: "Choose the next move" },
] as const;

function Showcase() {
  return (
    <aside className="showcase" aria-label="Why FinSight is different">
      <div className="showcase-rule" aria-hidden="true" />
      <div className="showcase-copy">
        <p className="showcase-kicker">Private, self-hosted finance</p>
        <h2>One calm place for the decisions that matter.</h2>
        <p>See the full picture, understand what changed, and plan your next move without turning your finances into a feed.</p>
      </div>
      <dl className="showcase-points">
        {SHOWCASE_POINTS.map((point) => (
          <div key={point.label}>
            <dt>{point.label}</dt>
            <dd>{point.detail}</dd>
          </div>
        ))}
      </dl>
      <div className="decision-loop" aria-label="FinSight decision loop">
        <div className="decision-loop-head">
          <span className="decision-loop-kicker">The FinSight loop</span>
          <span className="decision-loop-caption">A calmer way to decide</span>
        </div>
        <div className="decision-loop-track">
          <span className="decision-loop-line" aria-hidden="true" />
          {DECISION_STEPS.map((step, index) => (
            <div className="decision-step" data-step={index} key={step.label}>
              <span className="decision-step-index" aria-hidden="true">0{index + 1}</span>
              <strong>{step.label}</strong>
              <span>{step.detail}</span>
            </div>
          ))}
        </div>
      </div>
    </aside>
  );
}
/* ── Shell: brand, form, footer, and trust brief ─────────── */
export function AuthShell({
  eyebrow,
  title,
  subtitle,
  children,
}: {
  eyebrow: string;
  title: ReactNode;
  subtitle: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className="fs-auth">
      <div className="panel">
        <div className="brand">
          <div className="mark" />
          <div className="wm">Fin<b>Sight</b></div>
        </div>
        <div className="panel-body">
          <div className="head-eyebrow"><span className="dot" /> {eyebrow}</div>
          <h1 className="head-title">{title}</h1>
          <p className="head-sub">{subtitle}</p>
          {children}
        </div>
        <div className="panel-foot">
          <span className="lock">{Ico.lock({ style: { width: 13, height: 13 } })} 256-bit encrypted</span>
          <span>Local-first · self-hosted</span>
        </div>
      </div>
      <Showcase />
    </div>
  );
}

/** One-time recovery-key reveal, styled to the auth shell. The key lives only
 *  in props for the render's lifetime — it is never persisted client-side.
 *  Shared by SetupScreen (first-run) and RecoverScreen (post-reset). */
export function RecoveryReveal({ recoveryKey, onContinue }: { recoveryKey: string; onContinue: () => void }) {
  const [confirmed, setConfirmed] = useState(false);
  const [copied, setCopied] = useState(false);
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(recoveryKey);
      setCopied(true);
      toast.success("Recovery key copied");
    } catch {
      toast.error("Could not copy — select and copy the key manually");
    }
  };
  return (
    <div className="rk">
      <div className="rk-key">{recoveryKey}</div>
      <button type="button" className="btn-ghost" onClick={() => void copy()}>
        {copied ? "Copied" : "Copy to clipboard"}
      </button>
      <label className="rk-check">
        <input type="checkbox" checked={confirmed} onChange={(e) => setConfirmed(e.target.checked)} aria-label="I saved my recovery key" />
        <span>I saved my recovery key</span>
      </label>
      <button className="submit" type="button" style={{ marginTop: 18 }} disabled={!confirmed} onClick={onContinue}>
        Continue <span className="arw">{Ico.arrow()}</span>
      </button>
    </div>
  );
}
