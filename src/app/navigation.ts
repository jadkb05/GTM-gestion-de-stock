import type { IconName } from "../components/ui/Icon";

export const sections = [
  { id: "dashboard", label: "Tableau de bord", ready: true, icon: "dashboard" },
  { id: "catalogue", label: "Catalogue des produits", ready: true, icon: "package" },
  { id: "product-types", label: "Types de produits", ready: true, icon: "tag" },
  { id: "suppliers", label: "Fournisseurs", ready: true, icon: "truck" },
  { id: "entries", label: "Entrées de stock", ready: true, icon: "entry" },
  { id: "exits", label: "Sorties de stock", ready: true, icon: "exit" },
  { id: "history", label: "Historique des mouvements", ready: true, icon: "history" },
  { id: "sheet", label: "Fiche de stock", ready: true, icon: "sheet" },
  { id: "inventory", label: "Inventaires", ready: true, icon: "inventory" },
  { id: "alerts", label: "Réapprovisionnement", ready: true, icon: "reorder" },
  { id: "backup", label: "Sauvegarde", ready: true, icon: "backup" },
] as const satisfies readonly { id: string; label: string; ready: boolean; icon: IconName }[];

export type SectionId = (typeof sections)[number]["id"];

/* Regroupement de la barre latérale. La fiche de stock s'ouvre depuis les listes. */
export const navGroups: { label: string; items: SectionId[] }[] = [
  { label: "Pilotage", items: ["dashboard", "alerts"] },
  { label: "Opérations", items: ["entries", "exits", "inventory"] },
  { label: "Suivi", items: ["history"] },
  { label: "Référentiel", items: ["catalogue", "product-types", "suppliers"] },
  { label: "Données", items: ["backup"] },
];

export function sectionById(id: SectionId) {
  const section = sections.find((item) => item.id === id);
  if (!section) {
    throw new Error(`Module inconnu : ${id}`);
  }
  return section;
}
