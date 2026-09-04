import type { ReactNode } from "react";
import * as I from "../Icons";

interface MobileSectionProps {
  title?: string;
  description?: string;
  actionLabel?: string;
  onAction?: () => void;
  children: ReactNode;
  /** Optional extra class */
  className?: string;
}

export function MobileSection({
  title,
  description,
  actionLabel,
  onAction,
  children,
  className,
}: MobileSectionProps) {
  return (
    <section className={`mobile-section${className ? ` ${className}` : ""}`}>
      {title ? (
        <div className="mobile-section-head">
          <h2 className="mobile-section-title">{title}</h2>
          {actionLabel && onAction ? (
            <button type="button" className="mobile-section-action" onClick={onAction}>
              {actionLabel}
            </button>
          ) : null}
        </div>
      ) : null}
      {description ? <p className="mobile-section-desc">{description}</p> : null}
      {children}
    </section>
  );
}

interface MobileListProps {
  children: ReactNode;
  ariaLabel?: string;
  /** Compact inset — reserved for future grouped-inside-card use */
  inset?: boolean;
  className?: string;
}

export function MobileList({ children, ariaLabel, inset, className }: MobileListProps) {
  return (
    <div
      role={ariaLabel ? "list" : undefined}
      aria-label={ariaLabel}
      className={`mobile-list${inset ? " inset" : ""}${className ? ` ${className}` : ""}`}
    >
      {children}
    </div>
  );
}

interface MobileListItemProps {
  /** Left icon / avatar node. Typically <I.*> or a color dot. */
  icon?: ReactNode;
  title: string;
  subtitle?: string;
  /** Right-side primary value (e.g. "$42.00") — rendered with tabular nums */
  value?: string;
  valueTone?: "default" | "positive" | "negative";
  /** Smaller meta under value (e.g. "2 txns") */
  meta?: string;
  /** Show chevron */
  chevron?: boolean;
  /** Make the row interactive — renders as <button> */
  onPress?: () => void;
  /** Href for link rows */
  href?: string;
  /** Optional test id */
  testId?: string;
  /** Extra right node (e.g. a pill). Replaces chevron if present. */
  rightExtra?: ReactNode;
  children?: ReactNode;
}

export function MobileListItem({
  icon,
  title,
  subtitle,
  value,
  valueTone = "default",
  meta,
  chevron = true,
  onPress,
  href,
  testId,
  rightExtra,
  children,
}: MobileListItemProps) {
  const content = (
    <>
      <span className="mobile-list-item-left">
        {icon ? <span className="mobile-list-item-ico" aria-hidden="true">{icon}</span> : null}
        <span className="mobile-list-item-text">
          <span className="mobile-list-item-title">{title}</span>
          {subtitle ? <span className="mobile-list-item-subtitle">{subtitle}</span> : null}
          {children}
        </span>
      </span>
      {(value || meta || rightExtra) ? (
        <span className="mobile-list-item-right">
          {value ? (
            <span className={`mobile-list-item-value ${valueTone !== "default" ? valueTone : ""}`.trim()}>
              {value}
            </span>
          ) : null}
          {meta ? <span className="mobile-list-item-meta">{meta}</span> : null}
          {rightExtra}
        </span>
      ) : null}
      {chevron && (onPress || href) ? (
        <span className="mobile-list-item-chevron" aria-hidden="true">
          <I.ArrowRight width={14} height={14} />
        </span>
      ) : null}
    </>
  );

  const baseClass = "mobile-list-item";
  const a11yProps = {
    "data-testid": testId,
    "aria-label": `${title}${subtitle ? `, ${subtitle}` : ""}${value ? `, ${value}` : ""}`,
  };

  // An interactive control cannot carry role="listitem" (axe aria-allowed-role
  // violation: implicit button/link role wins), yet the parent MobileList is a
  // role="list" whose owned children must be listitems. The ARIA-valid shape
  // is a non-interactive listitem wrapper around the control. The wrapper is
  // visually transparent — the button keeps .mobile-list-item and fills it.
  if (href) {
    return (
      <div role="listitem">
        <a href={href} className={baseClass} {...a11yProps}>
          {content}
        </a>
      </div>
    );
  }

  if (onPress) {
    return (
      <div role="listitem">
        <button type="button" onClick={onPress} className={baseClass} {...a11yProps}>
          {content}
        </button>
      </div>
    );
  }

  return (
    <div className={baseClass} role="listitem" {...a11yProps}>
      {content}
    </div>
  );
}
