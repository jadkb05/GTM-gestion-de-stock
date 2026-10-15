import {
  createProductType,
  listProductTypes,
  setProductTypeActive,
  updateProductType,
} from "../catalogue/api";
import { ReferenceList } from "./ReferenceList";

function loadTypes() {
  return listProductTypes(true);
}

export function ProductTypesPage() {
  return (
    <ReferenceList
      title="Types de produits"
      addTitle="Nouveau type"
      note="Les types Silicone, Acrylique et Autre sont déjà présents. Le type « Vissable » n'est pas préchargé : son orthographe et sa signification restent à confirmer. Vous pouvez l'ajouter ici, sans modifier le code."
      emptyMessage="Aucun type de produit n'est enregistré."
      emptyNameMessage="Indiquez le nom du type."
      load={loadTypes}
      create={createProductType}
      update={updateProductType}
      setActive={setProductTypeActive}
    />
  );
}
