import type { ReactNode } from "react";

interface EmptyStateProps {
  icon?: ReactNode;
  title: string;
  description?: string;
  actions?: ReactNode;
  compact?: boolean;
}

export default function EmptyState({
  icon,
  title,
  description,
  actions,
  compact = false,
}: EmptyStateProps) {
  if (compact) {
    return (
      <div className="empty-panel">
        {icon && <div style={{ color: "var(--ink-mute)" }}>{icon}</div>}
        <h2>{title}</h2>
        {description && <p>{description}</p>}
        {actions && <div className="empty-actions">{actions}</div>}
      </div>
    );
  }

  return (
    <div className="empty-state">
      <div className="empty-panel">
        {icon && <div style={{ color: "var(--ink-mute)" }}>{icon}</div>}
        <h2>{title}</h2>
        {description && <p>{description}</p>}
        {actions && <div className="empty-actions">{actions}</div>}
      </div>
    </div>
  );
}
