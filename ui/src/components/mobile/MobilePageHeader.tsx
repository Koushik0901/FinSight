import type { ReactNode } from "react";

interface MobilePageHeaderProps {
  title: string;
  eyebrow?: string;
  description?: string;
  /** Optional action node (button, chip row). Rendered below description. */
  action?: ReactNode;
  /** Use ruled variant (bottom border) — e.g. Reports/Budget overview */
  ruled?: boolean;
  /** Optional dot accent before eyebrow (financial status) */
  withDot?: boolean;
}

export function MobilePageHeader({
  title,
  eyebrow,
  description,
  action,
  ruled = false,
  withDot = false,
}: MobilePageHeaderProps) {
  return (
    <header className={`mobile-page-header${ruled ? " mobile-page-header--ruled" : ""}`}>
      {eyebrow ? (
        <div className="mobile-page-eyebrow">
          {withDot ? <span className="dot" aria-hidden="true" /> : null}
          {eyebrow}
        </div>
      ) : null}
      <h1 className="mobile-page-title">{title}</h1>
      {description ? <p className="mobile-page-desc">{description}</p> : null}
      {action ? <div className="mobile-page-actions">{action}</div> : null}
    </header>
  );
}

export default MobilePageHeader;
