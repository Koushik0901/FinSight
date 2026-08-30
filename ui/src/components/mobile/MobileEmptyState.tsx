import type { ReactNode } from "react";

interface MobileEmptyStateProps {
  icon?: ReactNode;
  title: string;
  description?: string;
  primaryAction?: ReactNode;
  secondaryAction?: ReactNode;
}

export function MobileEmptyState({
  icon,
  title,
  description,
  primaryAction,
  secondaryAction,
}: MobileEmptyStateProps) {
  return (
    <div className="mobile-empty" role="status" aria-live="polite">
      {icon ? (
        <div className="mobile-empty-ico" aria-hidden="true">
          {icon}
        </div>
      ) : null}
      <h2 className="mobile-empty-title">{title}</h2>
      {description ? <p className="mobile-empty-desc">{description}</p> : null}
      {(primaryAction || secondaryAction) ? (
        <div className="mobile-empty-actions">
          {primaryAction}
          {secondaryAction}
        </div>
      ) : null}
    </div>
  );
}

export default MobileEmptyState;
