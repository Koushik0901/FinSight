import { createElement, type ReactNode } from "react";

interface EmptyStateProps {
  icon?: ReactNode;
  title: string;
  description?: string;
  details?: ReactNode;
  actions?: ReactNode;
  compact?: boolean;
  headingLevel?: 1 | 2 | 3;
}

export default function EmptyState({
  icon,
  title,
  description,
  details,
  actions,
  compact = false,
  headingLevel = 2,
}: EmptyStateProps) {
  const heading = createElement(`h${headingLevel}`, null, title);

  if (compact) {
    return (
      <div className="empty-panel">
        {icon && <div style={{ color: "var(--ink-mute)" }}>{icon}</div>}
        {heading}
        {description && <p>{description}</p>}
        {details && <div className="empty-details">{details}</div>}
        {actions && <div className="empty-actions">{actions}</div>}
      </div>
    );
  }

  return (
    <div className="empty-state">
      <div className="empty-panel">
        {icon && <div style={{ color: "var(--ink-mute)" }}>{icon}</div>}
        {heading}
        {description && <p>{description}</p>}
        {details && <div className="empty-details">{details}</div>}
        {actions && <div className="empty-actions">{actions}</div>}
      </div>
    </div>
  );
}
