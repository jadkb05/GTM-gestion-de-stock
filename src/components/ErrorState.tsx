import { Icon } from "./ui/Icon";

type ErrorStateProps = {
  message: string;
  onRetry: () => void;
};

export function ErrorState({ message, onRetry }: ErrorStateProps) {
  return (
    <section role="alert" className="panel mt-6 max-w-xl overflow-hidden border-danger-line">
      <div className="flex gap-3 bg-danger-bg px-5 py-4 text-[#7a1810]">
        <Icon name="alert" size={20} className="mt-0.5 shrink-0 text-danger" />
        <div>
          <h1 className="text-section font-semibold">La base locale est indisponible</h1>
          <p className="mt-1 text-table">{message}</p>
        </div>
      </div>
      <div className="flex justify-end border-t border-danger-line px-5 py-3">
        <button type="button" onClick={onRetry} className="btn btn-primary">
          Réessayer
        </button>
      </div>
    </section>
  );
}
