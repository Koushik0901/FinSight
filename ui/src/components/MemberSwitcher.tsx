import { useHouseholdMembers } from "../api/hooks/household";

interface Props {
  /** Selected member id, or null for the whole household ("Everyone"). */
  value: string | null;
  onChange: (memberId: string | null) => void;
}

/**
 * Everyone / per-member segmented control. Renders nothing for a household with
 * fewer than two members (with one person, "Everyone" already IS that person).
 */
export default function MemberSwitcher({ value, onChange }: Props) {
  const { data: members = [] } = useHouseholdMembers();
  if (members.length < 2) return null;
  const active = { borderColor: "var(--accent)", color: "var(--accent)" } as const;
  return (
    <div className="row row-sm wrap" role="tablist" aria-label="Filter by household member">
      <button
        type="button"
        className="chip"
        role="tab"
        aria-selected={value === null}
        style={value === null ? active : undefined}
        onClick={() => onChange(null)}
      >
        Everyone
      </button>
      {members.map((m) => (
        <button
          key={m.id}
          type="button"
          className="chip"
          role="tab"
          aria-selected={value === m.id}
          style={value === m.id ? active : undefined}
          onClick={() => onChange(m.id)}
        >
          <span
            className="cswatch"
            style={{ background: m.color || "var(--ink-faint)", width: 8, height: 8, marginRight: 6 }}
          />
          {m.name}
        </button>
      ))}
    </div>
  );
}
