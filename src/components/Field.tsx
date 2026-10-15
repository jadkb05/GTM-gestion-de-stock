import type { ReactNode } from "react";

type FieldProps = {
  id: string;
  label: string;
  required?: boolean;
  error?: string;
  hint?: ReactNode;
  className?: string;
  children: ReactNode;
};

export function Field({ id, label, required = false, error, hint, className = "", children }: FieldProps) {
  return (
    <div className={className}>
      <div className="flex items-baseline gap-1">
        <label htmlFor={id} className="field-label">
          {label}
        </label>
        {required ? (
          <span className="text-meta font-semibold text-gtm-red-strong">
            <span aria-hidden="true">*</span>
            <span className="sr-only">obligatoire</span>
          </span>
        ) : null}
      </div>
      {children}
      {hint && !error ? <p className="mt-1 text-meta text-faint">{hint}</p> : null}
      {error ? (
        <p id={`${id}-error`} className="mt-1 text-meta font-medium text-danger">
          {error}
        </p>
      ) : null}
    </div>
  );
}

/* Libellé et contrôle d'un filtre, associés implicitement. */
export function FilterField({ label, children, className = "" }: { label: string; children: ReactNode; className?: string }) {
  return (
    <label className={`block min-w-0 ${className}`}>
      <span className="field-label">{label}</span>
      {children}
    </label>
  );
}

export function RequiredLegend() {
  return (
    <p className="text-meta text-faint">
      <span className="font-semibold text-gtm-red-strong" aria-hidden="true">
        *
      </span>{" "}
      Champ obligatoire
    </p>
  );
}

export const controlClass = "control";
export const smallControlClass = "control control-sm";
export const buttonClass = "btn btn-primary";
export const quietButtonClass = "btn btn-secondary";
export const smallButtonClass = "btn btn-secondary btn-sm";
export const ghostButtonClass = "btn btn-ghost btn-sm";
export const dangerButtonClass = "btn btn-danger";
