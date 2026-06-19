import { useNavigate } from "react-router-dom";
import * as I from "./Icons";

interface Props {
  /** Pre-filled question that will appear in the Copilot ask bar */
  prompt: string;
  /** Short label shown on the nudge card */
  label: string;
  /** Brief contextual description */
  description?: string;
  /** Visual tone */
  variant?: "accent" | "warning" | "info";
  /** Optional count badge (e.g. "3 overages") */
  count?: number;
}

/**
 * Compact contextual entry point into the Copilot workspace.
 * Navigates to /copilot and stores the pre-filled prompt in sessionStorage
 * so the screen can pick it up and pre-populate the ask bar.
 */
export function CopilotNudge({
  prompt,
  label,
  description,
  variant = "info",
  count,
}: Props) {
  const navigate = useNavigate();

  const colors = {
    accent: {
      bg: "var(--accent-2)",
      border: "var(--accent-3)",
      icon: "var(--accent)",
    },
    warning: {
      bg: "var(--warning-2)",
      border: "rgba(251,191,36,0.30)",
      icon: "var(--warning)",
    },
    info: {
      bg: "var(--surface-2)",
      border: "var(--line)",
      icon: "var(--ink-mute)",
    },
  }[variant];

  const handleClick = () => {
    // Store the prompt so Copilot.tsx can pre-populate the ask bar
    sessionStorage.setItem("copilot.prefill", prompt);
    navigate("/copilot");
  };

  return (
    <button
      onClick={handleClick}
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: 8,
        padding: "7px 12px",
        background: colors.bg,
        border: `1px solid ${colors.border}`,
        borderRadius: 8,
        cursor: "pointer",
        fontSize: 12.5,
        color: "var(--ink-mute)",
        transition: "background .12s, border-color .12s",
        textAlign: "left",
      }}
      title={`Ask Copilot: ${prompt}`}
    >
      <I.Brain width={13} height={13} style={{ color: colors.icon, flexShrink: 0 }} />
      <span style={{ fontWeight: 500, color: colors.icon }}>{label}</span>
      {count !== undefined && count > 0 && (
        <span
          style={{
            display: "inline-flex",
            alignItems: "center",
            justifyContent: "center",
            minWidth: 18,
            height: 18,
            padding: "0 5px",
            borderRadius: 999,
            background: colors.border,
            fontSize: 11,
            fontWeight: 600,
            color: colors.icon,
          }}
        >
          {count}
        </span>
      )}
      {description && (
        <span style={{ color: "var(--ink-faint)", fontSize: 11.5 }}>— {description}</span>
      )}
      <I.ArrowRight width={11} height={11} style={{ marginLeft: 2, color: "var(--ink-faint)" }} />
    </button>
  );
}
