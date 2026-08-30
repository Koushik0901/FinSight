import { useState } from "react";
import * as I from "../../components/Icons";
import { MobileSection, MobileList, MobileListItem } from "../../components/mobile/MobileList";
import { BottomSheet } from "../../components/mobile/BottomSheet";
import { Toggle } from "../../components/Toggle";
import { SegmentedControl } from "../../components/mobile/SegmentedControl";
import { useTweaks, ACCENTS } from "../../state/tweaks";
import type { AccentId } from "../../state/tweaks";
import { useDefaultCurrency, useAutoCategorizeEnabled, useSetAutoCategorizeEnabled } from "../../api/hooks/settings";
import { isServerMode } from "../../api/auth";
import SettingsAccounts from "../settings/SettingsAccounts";
import SettingsAppearance from "../settings/SettingsAppearance";
import SettingsCurrency from "../settings/SettingsCurrency";
import SettingsData, { PhilosophySection } from "../settings/SettingsData";
import SettingsShortcuts from "../settings/SettingsShortcuts";

type SheetKey =
  | "profile"
  | "philosophy"
  | "data"
  | "agent"
  | "provider"
  | "appearance"
  | "currency"
  | "connections"
  | "notifications"
  | "keyboard"
  | "about"
  | "accent";

export default function MobileSettings() {
  const { theme, density, accent, privacy, setTheme, setDensity, setAccent, setPrivacy } = useTweaks();
  const { data: currentCurrency = "USD" } = useDefaultCurrency();
  const { data: autoCatEnabled } = useAutoCategorizeEnabled();
  const setAutoCat = useSetAutoCategorizeEnabled();
  const serverMode = isServerMode();
  const [sheet, setSheet] = useState<SheetKey | null>(null);

  const accentLabel = accent.charAt(0).toUpperCase() + accent.slice(1);

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        gap: 16,
        padding: 16,
        paddingBottom: "calc(24px + env(safe-area-inset-bottom, 0px))",
      }}
    >
      <p className="muted" style={{ fontSize: 13, lineHeight: 1.45, margin: 0 }}>
        Personalise FinSight, manage data and providers — one tap at a time.
      </p>

      {/* ── You ── */}
      <MobileSection title="You" description="Profile & how advice finds you">
        <MobileList ariaLabel="You">
          <MobileListItem
            icon={<I.House width={16} height={16} />}
            title="Profile & account"
            subtitle={serverMode ? "Signed-in user & onboarding" : "Onboarding & reset"}
            onPress={() => setSheet("profile")}
          />
          <MobileListItem
            icon={<I.Goal width={16} height={16} />}
            title="Financial targets"
            subtitle="Goals, emergency fund & pace"
            onPress={() => setSheet("philosophy")}
          />
          <MobileListItem
            icon={<I.Heart width={16} height={16} />}
            title="How you want advice"
            subtitle="Risk, strategy & coaching style"
            onPress={() => setSheet("philosophy")}
          />
        </MobileList>
      </MobileSection>

      {/* ── Privacy & Data ── */}
      <MobileSection title="Privacy & data" description="Keep amounts private and your data portable">
        <MobileList ariaLabel="Privacy and data">
          <MobileListItem
            icon={privacy ? <I.EyeOff width={16} height={16} /> : <I.Eye width={16} height={16} />}
            title="Privacy mode"
            subtitle={privacy ? "Amounts blurred — tap to show" : "Amounts visible — tap to blur"}
            chevron={false}
            rightExtra={<Toggle checked={privacy} onChange={setPrivacy} ariaLabel="Toggle privacy mode" />}
          />
          <MobileListItem
            icon={<I.Box width={16} height={16} />}
            title="Data & backups"
            subtitle="Health, exports & backups"
            onPress={() => setSheet("data")}
          />
          <MobileListItem
            icon={<I.Bolt width={16} height={16} />}
            title="Auto-categorize"
            subtitle="Let the agent sort new transactions"
            chevron={false}
            rightExtra={
              <Toggle
                checked={Boolean(autoCatEnabled)}
                onChange={(v) => setAutoCat.mutate(v)}
                ariaLabel="Toggle auto-categorize"
              />
            }
          />
        </MobileList>
      </MobileSection>

      {/* ── Appearance ── */}
      <MobileSection title="Appearance" description="Theme, density and accent — tuned for thumb reach">
        <MobileList ariaLabel="Appearance">
          <MobileListItem
            icon={<I.Sparkle width={16} height={16} />}
            title="Theme"
            subtitle={`${theme === "dark" ? "Dark" : "Light"} mode`}
            chevron={false}
            rightExtra={
              <SegmentedControl
                ariaLabel="Theme"
                value={theme}
                onChange={(v) => setTheme(v as typeof theme)}
                options={[
                  { value: "dark", label: "Dark" },
                  { value: "light", label: "Light" },
                ]}
              />
            }
          />
          <MobileListItem
            icon={<I.Grid width={16} height={16} />}
            title="Density"
            subtitle={density === "cozy" ? "Cozy — spacious cards" : "Compact — more per screen"}
            chevron={false}
            rightExtra={
              <SegmentedControl
                ariaLabel="Density"
                value={density}
                onChange={(v) => setDensity(v as typeof density)}
                options={[
                  { value: "cozy", label: "Cozy" },
                  { value: "compact", label: "Compact" },
                ]}
              />
            }
          />
          <MobileListItem
            icon={<I.Spark width={16} height={16} />}
            title="Accent"
            subtitle={accentLabel}
            onPress={() => setSheet("accent")}
            rightExtra={
              <span
                aria-hidden="true"
                style={{
                  width: 20,
                  height: 20,
                  borderRadius: 999,
                  background: ACCENTS[accent].hex,
                  border: "1px solid var(--line)",
                  flexShrink: 0,
                }}
              />
            }
          />
          <MobileListItem
            icon={<I.Wallet width={16} height={16} />}
            title="Currency"
            subtitle={currentCurrency}
            onPress={() => setSheet("currency")}
          />
        </MobileList>
      </MobileSection>

      {/* ── Intelligence ── */}
      <MobileSection title="Intelligence" description="Agent, memory and AI provider">
        <MobileList ariaLabel="Intelligence">
          <MobileListItem
            icon={<I.Brain width={16} height={16} />}
            title="Agent"
            subtitle="Memory & actions"
            onPress={() => setSheet("agent")}
          />
          <MobileListItem
            icon={<I.Cpu width={16} height={16} />}
            title="AI provider"
            subtitle="Ollama / OpenAI / Anthropic"
            onPress={() => setSheet("provider")}
          />
        </MobileList>
      </MobileSection>

      {/* ── System ── */}
      <MobileSection title="System" description="Connections, notifications and app info">
        <MobileList ariaLabel="System">
          <MobileListItem
            icon={<I.Fork width={16} height={16} />}
            title="Connections"
            subtitle="Bank feeds & MCP"
            onPress={() => setSheet("connections")}
          />
          <MobileListItem
            icon={<I.Bell width={16} height={16} />}
            title="Notifications"
            subtitle="Push & policy"
            onPress={() => setSheet("notifications")}
          />
          <MobileListItem
            icon={<I.Grid width={16} height={16} />}
            title="Keyboard"
            subtitle="Shortcuts & command palette"
            onPress={() => setSheet("keyboard")}
          />
          <MobileListItem
            icon={<I.Info width={16} height={16} />}
            title="About"
            subtitle="Version & server"
            onPress={() => setSheet("about")}
          />
          {serverMode ? (
            <MobileListItem
              icon={<I.Lock width={16} height={16} />}
              title="Account"
              subtitle="Sessions & sign out"
              onPress={() => setSheet("profile")}
            />
          ) : null}
        </MobileList>
      </MobileSection>

      {/* ── Sheets — reuse desktop subcomponents, thumb-friendly bottom sheets ── */}
      <BottomSheet open={sheet === "profile"} onClose={() => setSheet(null)} title="Profile & account" fullHeight>
        <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
          <SettingsAccounts />
        </div>
      </BottomSheet>

      <BottomSheet open={sheet === "philosophy"} onClose={() => setSheet(null)} title="How you want advice" fullHeight>
        <div style={{ display: "flex", flexDirection: "column", gap: 24 }}>
          <PhilosophySection />
          <div style={{ borderTop: "1px solid var(--line)", paddingTop: 16 }}>
            <p className="muted" style={{ fontSize: 12, marginBottom: 12 }}>
              Financial targets are edited alongside philosophy in Settings.
            </p>
            <SettingsData />
          </div>
        </div>
      </BottomSheet>

      <BottomSheet open={sheet === "data"} onClose={() => setSheet(null)} title="Data & backups" fullHeight>
        <SettingsData />
      </BottomSheet>

      <BottomSheet open={sheet === "agent"} onClose={() => setSheet(null)} title="Agent" fullHeight>
        <SettingsData />
      </BottomSheet>

      <BottomSheet open={sheet === "provider"} onClose={() => setSheet(null)} title="AI provider" fullHeight>
        <SettingsData />
      </BottomSheet>

      <BottomSheet open={sheet === "appearance"} onClose={() => setSheet(null)} title="Appearance" fullHeight>
        <SettingsAppearance />
      </BottomSheet>

      <BottomSheet
        open={sheet === "accent"}
        onClose={() => setSheet(null)}
        title="Accent"
        description="Pick the accent used in hero states and active controls"
      >
        <div style={{ display: "flex", flexWrap: "wrap", gap: 12, padding: "8px 0" }}>
          {(Object.entries(ACCENTS) as [AccentId, { hex: string; ink: string }][]).map(([id, val]) => (
            <button
              key={id}
              type="button"
              aria-label={id}
              aria-pressed={accent === id}
              onClick={() => setAccent(id)}
              style={{
                width: 44,
                height: 44,
                borderRadius: 999,
                background: val.hex,
                border: accent === id ? "2px solid var(--ink)" : "1px solid var(--line)",
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
                flexShrink: 0,
              }}
            >
              {accent === id ? <I.Check width={16} height={16} style={{ color: val.ink }} /> : null}
            </button>
          ))}
        </div>
        <p className="muted" style={{ fontSize: 12, marginTop: 8 }}>Current: {accentLabel}</p>
      </BottomSheet>

      <BottomSheet open={sheet === "currency"} onClose={() => setSheet(null)} title="Currency">
        <SettingsCurrency />
      </BottomSheet>

      <BottomSheet open={sheet === "connections"} onClose={() => setSheet(null)} title="Connections" fullHeight>
        <SettingsShortcuts />
      </BottomSheet>

      <BottomSheet open={sheet === "notifications"} onClose={() => setSheet(null)} title="Notifications" fullHeight>
        <SettingsShortcuts />
      </BottomSheet>

      <BottomSheet open={sheet === "keyboard"} onClose={() => setSheet(null)} title="Keyboard" fullHeight>
        <SettingsShortcuts />
      </BottomSheet>

      <BottomSheet open={sheet === "about"} onClose={() => setSheet(null)} title="About" fullHeight>
        <SettingsShortcuts />
      </BottomSheet>
    </div>
  );
}
