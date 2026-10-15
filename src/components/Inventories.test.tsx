import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Inventories } from "./Inventories";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const item = {
  id: 4,
  name: "Inventaire mars",
  status: "draft",
  createdAt: "2026-03-02T09:15:00.000Z",
  validatedAt: null,
  productCount: 3,
  varianceCount: 1,
};

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("liste des inventaires", () => {
  it("affiche les inventaires et leur statut", async () => {
    vi.mocked(invoke).mockResolvedValue([item]);
    render(
      <Inventories
        inventoryId={null}
        onOpen={() => undefined}
        onBack={() => undefined}
        onOpenSheet={() => undefined}
        onStockChanged={() => undefined}
      />,
    );

    expect(await screen.findByText("Inventaire mars")).toBeTruthy();
    expect(screen.getByText("Brouillon")).toBeTruthy();
    expect(screen.getByText("3")).toBeTruthy();
    expect(screen.getByText("1")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Détail" })).toBeTruthy();
  });

  it("crée un inventaire", async () => {
    const onOpen = vi.fn();
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "create_inventory") {
        return { ...item, id: 9, lines: [], counts: emptyCounts() };
      }
      return [];
    });
    render(
      <Inventories
        inventoryId={null}
        onOpen={onOpen}
        onBack={() => undefined}
        onOpenSheet={() => undefined}
        onStockChanged={() => undefined}
      />,
    );
    fireEvent.click(await screen.findByRole("button", { name: "Nouvel inventaire" }));
    fireEvent.change(screen.getByLabelText("Nom ou référence"), { target: { value: "Zone A" } });
    fireEvent.click(screen.getByRole("button", { name: "Créer l'inventaire" }));

    await waitFor(() => {
      expect(onOpen).toHaveBeenCalledWith(9);
    });
    expect(invoke).toHaveBeenCalledWith("create_inventory", {
      inventory: { name: "Zone A", comment: null },
    });
  });

  it("affiche une erreur renvoyée par le backend", async () => {
    vi.mocked(invoke).mockRejectedValue("La base locale est indisponible.");
    render(
      <Inventories
        inventoryId={null}
        onOpen={() => undefined}
        onBack={() => undefined}
        onOpenSheet={() => undefined}
        onStockChanged={() => undefined}
      />,
    );
    expect(await screen.findByRole("alert")).toBeTruthy();
    expect(screen.getByText("La base locale est indisponible.")).toBeTruthy();
  });
});

function emptyCounts() {
  return { counted: 0, unchanged: 0, surplus: 0, shortage: 0, uncounted: 0, stale: 0 };
}
