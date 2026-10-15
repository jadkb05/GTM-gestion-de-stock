import { EmptyState, PageHeader } from "./ui/Layout";

type UpcomingModuleProps = {
  label: string;
};

export function UpcomingModule({ label }: UpcomingModuleProps) {
  return (
    <div>
      <PageHeader title={label} />
      <EmptyState title="Ce module n'est pas encore disponible.">
        Il sera ajouté dans une prochaine version. Aucune donnée n'est gérée ici.
      </EmptyState>
    </div>
  );
}
