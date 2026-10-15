import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { afterEach, describe, expect, it, vi } from "vitest";
import { AlertsPage } from "./AlertsPage";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const alert = {
  productId: 7,
  name: "Colle",
  reference: "COL-1",
  typeId: 3,
  typeName: "Autre",
  supplierId: 2,
  supplierName: "Selena",
  quantity: "3",
  threshold: "5",
  gap: "-2",
};

const type = { id: 3, name: "Autre", isActive: true, createdAt: "", updatedAt: "" };
const supplier = { id: 2, name: "Selena", isActive: true, createdAt: "", updatedAt: "" };

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

function renderPage(onOpenSheet = vi.fn(), onOpenEntry = vi.fn()) {
  return render(
    <AlertsPage onOpenSheet={onOpenSheet} onOpenEntry={onOpenEntry} onStockChanged={() => undefined} />,
  );
}

describe("réapprovisionnement", () => {
  it("affiche les produits en alerte", async () => {
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "list_reorder_alerts") return [alert];
      if (command === "list_product_types") return [type];
      if (command === "list_suppliers") return [supplier];
      return [];
    });
    renderPage();
    expect(await screen.findByText("Colle")).toBeTruthy();
    expect(screen.getByText("COL-1")).toBeTruthy();
    expect(screen.getAllByText("Selena").length).toBeGreaterThan(0);
    expect(screen.getByText("Sous le seuil de 2")).toBeTruthy();
  });

  it("affiche l'état vide", async () => {
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "list_reorder_alerts") return [];
      return [];
    });
    renderPage();
    expect(await screen.findByText("Aucun produit à réapprovisionner.")).toBeTruthy();
  });

  it("filtre et trie les alertes", async () => {
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "list_reorder_alerts") return [alert];
      if (command === "list_product_types") return [type];
      if (command === "list_suppliers") return [supplier];
      return [];
    });
    renderPage();
    await screen.findByText("Colle");
    fireEvent.change(screen.getByLabelText("Recherche"), { target: { value: "col" } });
    fireEvent.change(screen.getByLabelText("Type"), { target: { value: "3" } });
    fireEvent.change(screen.getByLabelText("Fournisseur"), { target: { value: "2" } });
    fireEvent.change(screen.getByLabelText("Tri"), { target: { value: "desc" } });

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("list_reorder_alerts", {
        query: { search: "col", typeId: 3, supplierId: 2, sort: "desc" },
      });
    });
  });

  it("modifie un seuil", async () => {
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "list_reorder_alerts") return [alert];
      if (command === "list_product_types") return [type];
      if (command === "list_suppliers") return [supplier];
      return null;
    });
    renderPage();
    const input = await screen.findByLabelText("Seuil de Colle");
    fireEvent.change(input, { target: { value: "4" } });
    fireEvent.click(screen.getByRole("button", { name: "Modifier le seuil de Colle" }));
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("set_reorder_threshold", { productId: 7, threshold: "4" });
    });
  });

  it("ouvre la fiche et l'entrée de stock", async () => {
    const onOpenSheet = vi.fn();
    const onOpenEntry = vi.fn();
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "list_reorder_alerts") return [alert];
      return [];
    });
    renderPage(onOpenSheet, onOpenEntry);
    fireEvent.click(await screen.findByRole("button", { name: "Fiche" }));
    fireEvent.click(screen.getByRole("button", { name: "Entrée de stock" }));
    expect(onOpenSheet).toHaveBeenCalledWith(7);
    expect(onOpenEntry).toHaveBeenCalledWith(7);
  });

  it("affiche une erreur backend", async () => {
    vi.mocked(invoke).mockRejectedValue("Les alertes n'ont pas pu être chargées.");
    renderPage();
    expect(await screen.findByRole("alert")).toBeTruthy();
    expect(screen.getByText("Les alertes n'ont pas pu être chargées.")).toBeTruthy();
  });
});
