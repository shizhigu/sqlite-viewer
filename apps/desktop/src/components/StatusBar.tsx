import { useAppStore } from "../store/app";

export function StatusBar() {
  const meta = useAppStore((s) => s.meta);
  // Deliberately no read/write chip here. The toolbar already has an
  // interactive one (click to toggle); duplicating a *non*-clickable
  // copy in the footer just made people wonder why clicking it did
  // nothing.
  return (
    <footer className="status">
      <span className="status__info">
        {meta
          ? `v${meta.sqlite_library_version} · ${meta.encoding} · ${meta.page_count} pg · ${formatBytes(meta.size_bytes)}`
          : "—"}
      </span>
    </footer>
  );
}

function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  if (n < 1024 * 1024 * 1024) return `${(n / 1024 / 1024).toFixed(1)} MB`;
  return `${(n / 1024 / 1024 / 1024).toFixed(2)} GB`;
}
