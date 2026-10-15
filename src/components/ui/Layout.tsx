import type { ReactNode } from "react";
import { Icon, type IconName } from "./Icon";

type PageHeaderProps = {
  title: ReactNode;
  description?: ReactNode;
  actions?: ReactNode;
  meta?: ReactNode;
  back?: { label: string; onClick: () => void };
};

/* En-tête commun à toutes les pages : retour éventuel, titre, contexte et actions. */
export function PageHeader({ title, description, actions, meta, back }: PageHeaderProps) {
  return (
    <header className="mb-5 border-b border-line pb-4">
      {back ? (
        <button type="button" className="btn btn-ghost btn-sm -ml-2 mb-2" onClick={back.onClick}>
          <Icon name="back" size={14} />
          {back.label}
        </button>
      ) : null}
      <div className="flex flex-wrap items-start justify-between gap-x-6 gap-y-3">
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
            <h1 className="text-title font-semibold text-ink">{title}</h1>
            {meta}
          </div>
          {description ? <div className="mt-1 text-table text-mist">{description}</div> : null}
        </div>
        {actions ? <div className="flex shrink-0 flex-wrap items-center gap-2">{actions}</div> : null}
      </div>
    </header>
  );
}

type PanelProps = {
  title?: ReactNode;
  actions?: ReactNode;
  children: ReactNode;
  className?: string;
  bodyClassName?: string;
  flush?: boolean;
  as?: "section" | "div" | "article";
  labelledBy?: string;
};

/* Surface blanche avec en-tête optionnel. `flush` retire le padding pour un tableau. */
export function Panel({
  title,
  actions,
  children,
  className = "",
  bodyClassName = "",
  flush = false,
  as: Tag = "section",
  labelledBy,
}: PanelProps) {
  return (
    <Tag className={`panel ${className}`} aria-labelledby={labelledBy}>
      {title || actions ? (
        <div className="panel-header">
          {title ? (
            <h2 id={labelledBy} className="panel-title">
              {title}
            </h2>
          ) : (
            <span />
          )}
          {actions ? <div className="flex flex-wrap items-center gap-2">{actions}</div> : null}
        </div>
      ) : null}
      <div className={flush ? bodyClassName : `panel-body ${bodyClassName}`}>{children}</div>
    </Tag>
  );
}

/* Barre de filtres alignée sur une grille régulière. */
export function FilterBar({ children, className = "" }: { children: ReactNode; className?: string }) {
  return (
    <div className={`mb-4 grid items-end gap-3 rounded-panel border border-line bg-surface p-3 ${className}`}>
      {children}
    </div>
  );
}

export function EmptyState({
  icon = "package",
  title,
  children,
  action,
}: {
  icon?: IconName;
  title: ReactNode;
  children?: ReactNode;
  action?: ReactNode;
}) {
  return (
    <div className="flex flex-col items-center justify-center rounded-panel border border-dashed border-line-strong bg-surface px-6 py-10 text-center">
      <span className="mb-3 flex size-10 items-center justify-center rounded-full bg-surface-sunken text-mist">
        <Icon name={icon} size={20} />
      </span>
      <p className="font-medium text-ink">{title}</p>
      {children ? <div className="mt-1 max-w-md text-table text-mist">{children}</div> : null}
      {action ? <div className="mt-4">{action}</div> : null}
    </div>
  );
}

export function Loading({ children, className = "" }: { children: ReactNode; className?: string }) {
  return (
    <p className={`flex items-center gap-2 py-3 text-table text-mist ${className}`} aria-busy="true">
      <span className="spinner text-gtm-blue" aria-hidden="true" />
      {children}
    </p>
  );
}

/* Paire libellé / valeur pour les fiches et récapitulatifs. */
export function Detail({ label, children, className = "" }: { label: ReactNode; children: ReactNode; className?: string }) {
  return (
    <div className={`min-w-0 ${className}`}>
      <dt className="text-meta text-mist">{label}</dt>
      <dd className="mt-0.5 break-words font-medium text-ink">{children}</dd>
    </div>
  );
}
