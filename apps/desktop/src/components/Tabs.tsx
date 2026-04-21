import type { TabKind } from "../store/app";
import { useAppStore } from "../store/app";

const TABS: { key: TabKind; label: string; accelerator: string }[] = [
  { key: "browse", label: "Browse", accelerator: "⌘1" },
  { key: "query", label: "Query", accelerator: "⌘2" },
  { key: "schema", label: "Schema", accelerator: "⌘3" },
];

export function Tabs() {
  const active = useAppStore((s) => s.activeTab);
  const setActive = useAppStore((s) => s.setActiveTab);
  return (
    <nav className="tabs" role="tablist">
      {TABS.map((t) => (
        <button
          key={t.key}
          role="tab"
          aria-selected={active === t.key}
          className={`tab ${active === t.key ? "tab--active" : ""}`}
          onClick={() => setActive(t.key)}
          title={t.accelerator}
        >
          {t.label}
        </button>
      ))}
    </nav>
  );
}
