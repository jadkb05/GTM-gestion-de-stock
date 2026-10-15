import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { afterEach, describe, expect, it, vi } from "vitest";
import { StockSheet } from "./StockSheet";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const sheet = {
  productId: 100,
  name: "Mastic façade",
  reference: "SIL-01",
  typeName: "Silicone",
  supplierName: "Selena",
  isActive: true,
  quantity: "12.25",
  movementCount: 120,
  entriesTotal: "15.5",
  exitsTotal: "3.25",
};

const line = {
  id: 80,
  createdAt: "2026-10-13T12:00:00.000Z",
  direction: "out",
  quantity: "1.5",
  balanceAfter: "12.25",
  reason: "Pose",
  documentReference: "BS-9",
  comment: "Équipe A",
};

function page(current = 1, total = 120) {
  return { lines: [line], page: current, pageSize: 50, total };
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("fiche de stock", () => {
  it("affiche les statistiques, les soldes et le format français", async () => {
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "get_stock_sheet") {
        return sheet;
      }
      if (command === "list_stock_sheet_movements") {
        return page();
      }
      return { ...line, productId: 100, productName: "Mastic façade", productReference: "SIL-01", balanceAfter: "12.25" };
    });
    render(<StockSheet productId={100} backLabel="Retour au catalogue" onBack={() => undefined} />);
    expect(await screen.findByRole("heading", { name: "Mastic façade" })).toBeTruthy();
    expect(screen.getByText("SIL-01")).toBeTruthy();
    expect(screen.getByText("Silicone")).toBeTruthy();
    expect(screen.getByText("Selena")).toBeTruthy();
    expect(screen.getByText("Actif")).toBeTruthy();
    expect(screen.getAllByText("12,25").length).toBeGreaterThan(0);
    expect(screen.getByText("15,5")).toBeTruthy();
    expect(screen.getByText("3,25")).toBeTruthy();
    expect(screen.getByText("120")).toBeTruthy();
    expect(screen.getByText("Sortie")).toBeTruthy();
    expect(screen.getByText("1,5")).toBeTruthy();
    expect(screen.getByText(/1–1 sur 120 mouvements/)).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Détail" }));
    expect(await screen.findByText("Équipe A")).toBeTruthy();
    expect(screen.getByText("Solde après mouvement")).toBeTruthy();
    expect(invoke).toHaveBeenCalledWith("get_stock_movement", { id: 80 });
  });

  it("demande la page suivante sans relancer le calcul affiché", async () => {
    vi.mocked(invoke).mockImplementation(async (command: string, args?: unknown) => {
      const pageNumber =
        args && typeof args === "object" && "query" in args
          ? Number((args as { query?: { page?: number } }).query?.page ?? 1)
          : 1;
      if (command === "get_stock_sheet") {
        return sheet;
      }
      if (command === "list_stock_sheet_movements") {
        return page(pageNumber);
      }
      return line;
    });
    render(<StockSheet productId={100} backLabel="Retour au catalogue" onBack={() => undefined} />);
    fireEvent.click(await screen.findByRole("button", { name: "Page suivante" }));
    expect(await screen.findByText(/Page 2 sur 3/)).toBeTruthy();
    expect(invoke).toHaveBeenCalledWith("list_stock_sheet_movements", {
      query: {
        productId: 100,
        direction: "all",
        search: "",
        fromDate: null,
        toDate: null,
        page: 2,
      },
    });
  });

  it("transmet les filtres et signale une période inversée", async () => {
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "get_stock_sheet") {
        return sheet;
      }
      return { lines: [], page: 1, pageSize: 50, total: 0 };
    });
    render(<StockSheet productId={100} backLabel="Retour au catalogue" onBack={() => undefined} />);
    await screen.findByRole("heading", { name: "Mastic façade" });
    fireEvent.change(screen.getByLabelText("Motif ou référence"), { target: { value: "BS-9" } });
    fireEvent.change(screen.getByLabelText("Mouvement"), { target: { value: "out" } });
    expect(await screen.findByText("Aucun mouvement ne correspond à ces critères.")).toBeTruthy();
    expect(invoke).toHaveBeenCalledWith("list_stock_sheet_movements", {
      query: {
        productId: 100,
        direction: "out",
        search: "BS-9",
        fromDate: null,
        toDate: null,
        page: 1,
      },
    });
    fireEvent.change(screen.getByLabelText("Du"), { target: { value: "2026-05-02" } });
    fireEvent.change(screen.getByLabelText("Au"), { target: { value: "2026-01-01" } });
    expect(await screen.findByText("La date de fin précède la date de début.")).toBeTruthy();
  });

  it("indique un historique vide et reste lisible pour un produit désactivé", async () => {
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "get_stock_sheet") {
        return { ...sheet, isActive: false, quantity: "4", movementCount: 0, entriesTotal: "0", exitsTotal: "0" };
      }
      return { lines: [], page: 1, pageSize: 50, total: 0 };
    });
    render(<StockSheet productId={100} backLabel="Retour au catalogue" onBack={() => undefined} />);
    expect(await screen.findByText("Désactivé")).toBeTruthy();
    expect(screen.getByText("Aucun mouvement n'est enregistré pour ce produit.")).toBeTruthy();
    expect(screen.getByText(/fiche et l'historique restent consultables/)).toBeTruthy();
  });

  it("affiche l'erreur renvoyée par le backend", async () => {
    vi.mocked(invoke).mockRejectedValue("Ce produit est introuvable.");
    render(<StockSheet productId={999} backLabel="Retour au catalogue" onBack={() => undefined} />);
    expect((await screen.findByRole("alert")).textContent).toContain("Ce produit est introuvable.");
  });

  it("enregistre un seuil de réapprovisionnement", async () => {
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "get_stock_sheet") {
        return { ...sheet, reorderThreshold: null };
      }
      if (command === "list_stock_sheet_movements") {
        return page(1, 0);
      }
      return null;
    });
    render(<StockSheet productId={100} backLabel="Retour au catalogue" onBack={() => undefined} />);
    expect(await screen.findByText(/ne participe pas aux alertes/)).toBeTruthy();
    fireEvent.change(screen.getByLabelText("Seuil de réapprovisionnement"), { target: { value: "2,5" } });
    fireEvent.click(screen.getByRole("button", { name: "Enregistrer le seuil" }));
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("set_reorder_threshold", { productId: 100, threshold: "2.5" });
    });
  });
});
