import {
  createSupplier,
  listSuppliers,
  setSupplierActive,
  updateSupplier,
} from "../catalogue/api";
import { ReferenceList } from "./ReferenceList";

function loadSuppliers() {
  return listSuppliers(true);
}

export function SuppliersPage() {
  return (
    <ReferenceList
      title="Fournisseurs"
      addTitle="Nouveau fournisseur"
      note="Les fournisseurs Selena, Tremco et Legallais sont déjà présents. Le nom « Tenois » n'est pas préchargé : son orthographe reste à confirmer. Vous pouvez l'ajouter ici, sans modifier le code."
      emptyMessage="Aucun fournisseur n'est enregistré."
      emptyNameMessage="Indiquez le nom du fournisseur."
      load={loadSuppliers}
      create={createSupplier}
      update={updateSupplier}
      setActive={setSupplierActive}
    />
  );
}
