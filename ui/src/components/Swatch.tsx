interface SwatchProps {
  color: string;
  selected?: boolean;
  onClick?: () => void;
  label?: string;
}

export default function Swatch({ color, selected = false, onClick, label }: SwatchProps) {
  return (
    <button
      type="button"
      className={`swatch-btn ${selected ? "on" : ""}`}
      onClick={onClick}
      aria-label={label ?? `Choose color ${color}`}
      aria-pressed={selected}
      style={selected ? { borderColor: "var(--ink)" } : undefined}
    >
      <span className="dot" style={{ backgroundColor: color }} />
    </button>
  );
}
