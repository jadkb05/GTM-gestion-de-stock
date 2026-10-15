import type { ReactNode } from "react";

export type BadgeTone = "neutral" | "info" | "ok" | "warn" | "danger";

/* Statut lisible : la couleur accompagne toujours un libellé explicite. */
export function StatusBadge({ tone, children, dot = true }: { tone: BadgeTone; children: ReactNode; dot?: boolean }) {
  return (
    <span className={`badge badge-${tone}`}>
      {dot ? <span className="badge-dot" aria-hidden="true" /> : null}
      {children}
    </span>
  );
}
