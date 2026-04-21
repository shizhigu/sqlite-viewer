import { useAppStore } from "../store/app";

export function StatusBar() {
  const meta = useAppStore((s) => s.meta);
  const rw = useAppStore((s) => s.readWrite);
  return (
    <footer className="status">
      <span className={`chip ${rw ? "chip--mode-rw" : "chip--mode-ro"}`}>
        {rw ? "READ-WRITE" : "READ-ONLY"}
      </span>
      <span className="right">
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
