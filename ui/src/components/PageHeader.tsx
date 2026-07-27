import type { HTMLAttributes, ReactNode } from "react";

type PageHeaderVariant = "default" | "ruled";

interface PageHeaderProps extends Omit<HTMLAttributes<HTMLElement>, "title"> {
  eyebrow: ReactNode;
  title: ReactNode;
  description?: ReactNode;
  actions?: ReactNode;
  dot?: boolean;
  variant?: PageHeaderVariant;
}

/**
 * The consistent top-level heading for FinSight screens.
 *
 * Use `variant="ruled"` when the header should visually separate itself from
 * the screen body. Section headings inside a screen remain local compositions.
 */
export default function PageHeader({
  eyebrow,
  title,
  description,
  actions,
  dot = true,
  variant = "default",
  className = "",
  ...rest
}: PageHeaderProps) {
  const classes = [
    "page-header",
    variant === "ruled" ? "page-header-ruled" : "",
    className,
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <header className={classes} {...rest}>
      <div className="page-header-main">
        <div className="page-header-eyebrow">
          {dot && <span className="dot" aria-hidden="true" />}
          {eyebrow}
        </div>
        <h1 className="page-header-title">{title}</h1>
        {description && <p className="page-header-description">{description}</p>}
      </div>
      {actions && <div className="page-header-actions">{actions}</div>}
    </header>
  );
}
