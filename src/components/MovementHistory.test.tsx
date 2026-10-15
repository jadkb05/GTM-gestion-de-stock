import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { afterEach, describe, expect, it, vi } from "vitest";
import { MovementHistory } from "./MovementHistory";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const entry = {
  id: 1,
  productId: 100,
  productName: "Mastic façade",
  productReference: "SIL-01",
  direction: "in",
  quantity: "1.5",
  reason: "Réception",
  documentReference: "BL-10",
  comment: "Quai 2",
  createdAt: "2026-10-13T12:00:00.000Z",
};

const exitMovement = {
  ...entry,
  id: 2,
  direction: "out",
  quantity: "0.5",
  reason: "Chantier",
  documentReference: null,
  comment: null,
  createdAt: "2026-10-13T13:00:00.000Z",
};

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("historique des mouvements", () => {
  it("affiche les entrées et les sorties", async () => {
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "list_products") {
        return [{ id: 100, name: "Mastic façade", reference: "SIL-01", typeId: 1, typeName: "Silicone", supplierId: null, supplierName: null, description: null, isActive: true, createdAt: "", updatedAt: "" }];
      }
      if (command === "list_stock_movements") {
        return [exitMovement, entry];
      }
      return entry;
    });
    render(<MovementHistory />);
    expect(await screen.findByText("Entrée")).toBeTruthy();
    expect(screen.getByText("Sortie")).toBeTruthy();
    expect(screen.getByText("1,5")).toBeTruthy();
    expect(screen.getByText("BL-10")).toBeTruthy();
    fireEvent.click(screen.getAllByRole("button", { name: "Détail" })[1]);
    expect(await screen.findByRole("heading", { name: "Détail du mouvement" })).toBeTruthy();
    expect(screen.getByText("Quai 2")).toBeTruthy();
    const openSheet = vi.fn();
    cleanup();
    render(<MovementHistory onOpenSheet={openSheet} />);
    fireEvent.click((await screen.findAllByRole("button", { name: "Mastic façade" }))[0]);
    expect(openSheet).toHaveBeenCalledWith(100);
  });

  it("indique l'absence de mouvement et transmet les filtres", async () => {
    vi.mocked(invoke).mockResolvedValue([]);
    render(<MovementHistory />);
    expect(await screen.findByText("Aucun mouvement n'est enregistré.")).toBeTruthy();
    fireEvent.change(screen.getByLabelText("Recherche"), { target: { value: "bl" } });
    fireEvent.change(screen.getByLabelText("Mouvement"), { target: { value: "out" } });
    expect(await screen.findByText("Aucun mouvement ne correspond à ces critères.")).toBeTruthy();
    expect(invoke).toHaveBeenCalledWith("list_stock_movements", {
      query: { search: "bl", productId: null, direction: "out" },
    });
  });

  it("affiche une erreur de chargement", async () => {
    vi.mocked(invoke).mockRejectedValue("Le mouvement n'a pas pu être enregistré.");
    render(<MovementHistory />);
    expect((await screen.findByRole("alert")).textContent).toContain(
      "Le mouvement n'a pas pu être enregistré.",
    );
  });
});
