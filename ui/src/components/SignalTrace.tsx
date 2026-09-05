type SignalTraceVariant = "history" | "goals" | "reports";

const PATHS: Record<SignalTraceVariant, string> = {
  history: "M16 94 C54 84 68 62 104 68 S164 92 194 52 S232 38 264 22",
  goals: "M16 96 C54 96 72 82 102 82 S142 58 166 62 S214 36 264 28",
  reports: "M16 94 H68 V78 H116 V78 H116 V60 H166 V60 H166 V42 H216 V42 H216 V28 H264",
};

export default function SignalTrace({ variant = "history", label = "Waiting for a signal" }: { variant?: SignalTraceVariant; label?: string }) {
  const path = PATHS[variant];

  return (
    <div className={`signal-trace signal-trace-${variant}`} aria-hidden="true">
      <svg viewBox="0 0 280 120" role="presentation">
        <path className="signal-trace-grid" d="M16 22H264M16 58H264M16 94H264" />
        <path className="signal-trace-axis" d="M16 14V102M16 102H264" />
        <path className="signal-trace-line" pathLength="1" d={path} />
        <circle className="signal-trace-halo" cx="264" cy={variant === "history" ? 22 : variant === "goals" ? 28 : 28} r="13" />
        <circle className="signal-trace-point" cx="264" cy={variant === "history" ? 22 : variant === "goals" ? 28 : 28} r="4" />
      </svg>
      <span>{label}</span>
    </div>
  );
}
