import type { ReactNode } from "react";
import { Icon, type IconName } from "./Icon";

type Tone = "info" | "ok" | "warn" | "danger" | "neutral";

const icons: Record<Tone, IconName> = {
  info: "info",
  ok: "success",
  warn: "warning",
  danger: "alert",
  neutral: "info",
};

type CalloutProps = {
  tone: Tone;
  title?: ReactNode;
  children?: ReactNode;
  role?: "alert" | "status";
  action?: ReactNode;
  className?: string;
};

/* Message contextuel : succès, avertissement, erreur ou information. */
export function Callout({ tone, title, children, role, action, className = "" }: CalloutProps) {
  return (
    <div role={role} className={`callout callout-${tone} ${className}`}>
      <Icon name={icons[tone]} size={16} />
      <div className="min-w-0 flex-1">
        {title ? <p className="font-semibold">{title}</p> : null}
        {children ? <div className={title ? "mt-0.5" : undefined}>{children}</div> : null}
        {action ? <div className="mt-2">{action}</div> : null}
      </div>
    </div>
  );
}

export function Spinner() {
  return <span className="spinner" aria-hidden="true" />;
}

/* Libellé d'un mouvement de stock, avec une icône de sens. */
export function MovementTag({ label, direction }: { label: string; direction: "in" | "out" }) {
  const adjust = label === "Correction d'inventaire";
  const kind = adjust ? "movement-adjust" : direction === "in" ? "movement-in" : "movement-out";
  return (
    <span className={`movement ${kind}`}>
      <Icon name={adjust ? "adjust" : direction === "in" ? "entry" : "exit"} size={14} />
      <span>{label}</span>
    </span>
  );
}
