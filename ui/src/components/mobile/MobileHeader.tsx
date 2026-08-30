import type { ReactNode } from "react";
import { useNavigate } from "react-router-dom";
import * as I from "../Icons";

interface MobileHeaderProps {
  title: string;
  eyebrow?: string;
  /** Show back chevron (for drill-down screens). Defaults to false (root tabs). */
  showBack?: boolean;
  onBack?: () => void;
  actions?: ReactNode;
}

export function MobileHeader({ title, eyebrow, showBack = false, onBack, actions }: MobileHeaderProps) {
  const navigate = useNavigate();

  const handleBack = () => {
    if (onBack) onBack();
    else navigate(-1);
  };

  return (
    <header className="mobile-header" role="banner">
      <div style={{ display: "flex", alignItems: "center", gap: 10, minWidth: 0, flex: 1 }}>
        {showBack ? (
          <button
            type="button"
            className="mobile-header-icon-btn"
            aria-label="Back"
            onClick={handleBack}
          >
            <I.ArrowLeft width={16} height={16} aria-hidden="true" />
          </button>
        ) : null}
        <div className="mobile-header-titles">
          {eyebrow ? <span className="mobile-header-eyebrow">{eyebrow}</span> : null}
          <span className="mobile-header-title">{title}</span>
        </div>
      </div>
      {actions ? <div className="mobile-header-actions">{actions}</div> : null}
    </header>
  );
}

export default MobileHeader;
