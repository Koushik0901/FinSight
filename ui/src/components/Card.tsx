import type { HTMLAttributes, ReactNode } from "react";

type CardTone = "default" | "accent" | "warn" | "muted";

interface CardProps extends HTMLAttributes<HTMLDivElement> {
  children: ReactNode;
  tone?: CardTone;
  tight?: boolean;
  flush?: boolean;
  hover?: boolean;
  header?: ReactNode;
  footer?: ReactNode;
}

export default function Card({
  children,
  tone = "default",
  tight = false,
  flush = false,
  hover = false,
  header,
  footer,
  className = "",
  ...rest
}: CardProps) {
  const classes = [
    "card",
    tone !== "default" ? tone : "",
    tight ? "tight" : "",
    flush ? "flush" : "",
    hover ? "hover" : "",
    className,
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <div className={classes} {...rest}>
      {header && <div className="card-head">{header}</div>}
      <div className={flush ? "" : "card-body"}>{children}</div>
      {footer && <div className="card-foot">{footer}</div>}
    </div>
  );
}
