import type { ReactNode } from "react";

interface MobileStatProps {
  label: string;
  value: string;
  sub?: string | ReactNode;
  tone?: "default" | "accent" | "positive" | "negative";
  size?: "sm" | "default" | "lg";
  /** Hero variant — subtle accent wash for anchoring stat (net worth, remaining) */
  hero?: boolean;
  // Locale-aware privacy: value gets className="money" automatically
  privacyBlur?: boolean;
}

export function MobileStat({
  label,
  value,
  sub,
  size = "default",
  hero = false,
}: MobileStatProps) {
  return (
    <div className={`mobile-stat${hero ? " hero" : ""}`}>
      <span className="mobile-stat-label">{label}</span>
      <span className={`mobile-stat-value ${size !== "default" ? size : ""} money`.trim()}>
        {value}
      </span>
      {sub ? <span className="mobile-stat-sub">{sub}</span> : null}
    </div>
  );
}

interface MobileStatRowProps {
  children: ReactNode;
}

export function MobileStatRow({ children }: MobileStatRowProps) {
  return <div className="mobile-stat-row">{children}</div>;
}

export default MobileStat;
