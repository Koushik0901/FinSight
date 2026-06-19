import type { ReactNode } from "react";

type BadgeTone = "default" | "accent" | "positive" | "negative" | "warning";

interface BadgeProps {
  children: ReactNode;
  tone?: BadgeTone;
  className?: string;
  dot?: boolean;
}

export default function Badge({ children, tone = "default", className = "", dot = false }: BadgeProps) {
  const classes = ["badge", tone !== "default" ? tone : "", className].filter(Boolean).join(" ");

  return (
    <span className={classes}>
      {dot && <span className="dot" />}
      {children}
    </span>
  );
}
