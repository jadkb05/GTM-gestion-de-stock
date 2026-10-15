import { useEffect, useId, useRef, type ReactNode } from "react";
import { Icon } from "./Icon";

type DialogProps = {
  title: ReactNode;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
  size?: "sm" | "md" | "lg";
  closeDisabled?: boolean;
};

const widths = {
  sm: "max-w-md",
  md: "max-w-xl",
  lg: "max-w-3xl",
};

const focusable =
  'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

/* Fenêtre modale : focus initial, Échap pour fermer, tabulation contenue, focus restitué. */
export function Dialog({ title, onClose, children, footer, size = "md", closeDisabled = false }: DialogProps) {
  const titleId = useId();
  const panel = useRef<HTMLDivElement>(null);
  const closeRef = useRef(onClose);
  closeRef.current = onClose;
  const lockedRef = useRef(closeDisabled);
  lockedRef.current = closeDisabled;

  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    const node = panel.current;
    const first =
      node?.querySelector<HTMLElement>("[data-autofocus]") ??
      node?.querySelector<HTMLElement>(".panel-body input:not([disabled]), .panel-body select:not([disabled]), .panel-body textarea:not([disabled])");
    (first ?? node)?.focus();

    function onKey(event: KeyboardEvent) {
      if (event.key === "Escape" && !lockedRef.current) {
        event.preventDefault();
        closeRef.current();
        return;
      }
      if (event.key !== "Tab" || !node) {
        return;
      }
      const items = Array.from(node.querySelectorAll<HTMLElement>(focusable));
      if (items.length === 0) {
        return;
      }
      const head = items[0];
      const tail = items[items.length - 1];
      if (event.shiftKey && document.activeElement === head) {
        event.preventDefault();
        tail.focus();
      } else if (!event.shiftKey && document.activeElement === tail) {
        event.preventDefault();
        head.focus();
      }
    }

    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("keydown", onKey);
      previous?.focus?.();
    };
  }, []);

  return (
    <div className="fixed inset-0 z-40 flex items-start justify-center overflow-y-auto bg-graphite/45 px-4 py-[8vh]">
      <div
        ref={panel}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        tabIndex={-1}
        className={`panel w-full ${widths[size]} shadow-[0_12px_32px_rgb(17_24_31/0.22)] outline-none`}
      >
        <div className="panel-header">
          <h2 id={titleId} className="panel-title">
            {title}
          </h2>
          <button
            type="button"
            className="btn btn-ghost btn-sm -mr-1.5 px-1.5 text-mist"
            aria-label="Fermer"
            disabled={closeDisabled}
            onClick={onClose}
          >
            <Icon name="close" size={16} />
          </button>
        </div>
        <div className="panel-body">{children}</div>
        {footer ? (
          <div className="flex flex-wrap items-center justify-end gap-2 rounded-b-panel border-t border-line bg-surface-muted px-4 py-3">
            {footer}
          </div>
        ) : null}
      </div>
    </div>
  );
}
