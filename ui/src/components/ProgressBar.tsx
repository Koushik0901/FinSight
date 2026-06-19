interface ProgressBarProps {
  value: number;
  max?: number;
  size?: "sm" | "default" | "lg";
  tone?: "default" | "negative" | "warning";
  className?: string;
  "aria-label"?: string;
}

export default function ProgressBar({
  value,
  max = 100,
  size = "default",
  tone = "default",
  className = "",
  "aria-label": ariaLabel,
}: ProgressBarProps) {
  const pct = Math.max(0, Math.min(100, (value / max) * 100));
  const classes = ["progress-bar", size !== "default" ? size : "", tone !== "default" ? tone : "", className]
    .filter(Boolean)
    .join(" ");

  return (
    <div
      className={classes}
      role="progressbar"
      aria-valuenow={Math.round(value)}
      aria-valuemin={0}
      aria-valuemax={max}
      aria-label={ariaLabel}
    >
      <span style={{ width: `${pct}%` }} />
    </div>
  );
}
