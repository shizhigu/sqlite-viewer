import { useEffect } from "react";

import { useAppStore } from "../store/app";

// Auto-dismiss durations. Errors linger longer so the user has time to read
// them, but they do time out — a permanently sticky error pile is worse than
// a missed one (the agent / console logs are the source of truth anyway).
const DURATION_MS: Record<"info" | "success" | "error", number> = {
  info: 4_000,
  success: 4_000,
  error: 8_000,
};

export function Toasts() {
  const toasts = useAppStore((s) => s.toasts);
  const dismiss = useAppStore((s) => s.dismissToast);

  useEffect(() => {
    const timers = toasts.map((t) =>
      setTimeout(() => dismiss(t.id), DURATION_MS[t.kind]),
    );
    return () => {
      timers.forEach(clearTimeout);
    };
  }, [toasts, dismiss]);

  return (
    <div className="toasts">
      {toasts.map((t) => (
        <div
          key={t.id}
          className={`toast toast--${t.kind}`}
          role={t.kind === "error" ? "alert" : "status"}
          onClick={() => dismiss(t.id)}
          title="Click to dismiss"
        >
          {t.text}
        </div>
      ))}
    </div>
  );
}
