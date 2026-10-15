import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { afterEach, describe, expect, it, vi } from "vitest";
import { InventoryDetail } from "./InventoryDetail";
import type { InventoryDetail as InventoryData } from "../inventory/api";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

function detail(overrides: Partial<InventoryData> = {}): InventoryData {
  return {
    id: 4,
    name: "Inventaire mars",
    comment: null,
    status: "draft",
    createdAt: "2026-03-02T09:15:00.000Z",
    validatedAt: null,
    lines: [
      {
        id: 11,
        productId: 1,
        productName: "Colle",
        productReference: "COL-1",
        typeName: "Autre",
        theoretical: "10",
        counted: null,
        current: "10",
        variance: null,
        drifted: false,
      },
      {
        id: 12,
        productId: 2,
        productName: "Mastic",
        productReference: "MAS-1",
        typeName: "Silicone",
        theoretical: "12.5",
        counted: "10",
        current: "12.5",
        variance: "-2.5",
        drifted: false,
      },
    ],
    counts: { counted: 1, unchanged: 0, surplus: 0, shortage: 1, uncounted: 1, stale: 0 },
    ...overrides,
  };
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

function renderDetail() {
  return render(
    <InventoryDetail
      inventoryId={4}
      onBack={() => undefined}
      onOpenSheet={() => undefined}
      onStockChanged={() => undefined}
    />,
  );
}

describe("saisie d'un inventaire", () => {
  it("saisit plusieurs produits et affiche les écarts", async () => {
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "get_inventory") {
        return detail();
      }
      if (command === "save_inventory_counts") {
        return detail({
          lines: [
            {
              id: 11,
              productId: 1,
              productName: "Colle",
              productReference: "COL-1",
              typeName: "Autre",
              theoretical: "10",
              counted: "10",
              current: "10",
              variance: "0",
              drifted: false,
            },
            {
              id: 12,
              productId: 2,
              productName: "Mastic",
              productReference: "MAS-1",
              typeName: "Silicone",
              theoretical: "12.5",
              counted: "10",
              current: "12.5",
              variance: "-2.5",
              drifted: false,
            },
          ],
          counts: { counted: 2, unchanged: 1, surplus: 0, shortage: 1, uncounted: 0, stale: 0 },
        });
      }
      return [];
    });
    renderDetail();

    expect(await screen.findByText("Manque de 2,5")).toBeTruthy();
    expect(screen.getAllByText("Non compté").length).toBeGreaterThan(0);
    expect(screen.getByText("Des produits ne sont pas encore comptés.")).toBeTruthy();
    fireEvent.change(screen.getByLabelText("Quantité comptée de Colle"), { target: { value: "10" } });
    fireEvent.change(screen.getByLabelText("Quantité comptée de Mastic"), { target: { value: "10" } });
    fireEvent.click(screen.getByRole("button", { name: "Enregistrer les quantités" }));

    expect(await screen.findByText("Écart nul")).toBeTruthy();
    expect(invoke).toHaveBeenCalledWith(
      "save_inventory_counts",
      expect.objectContaining({
        inventoryId: 4,
        counts: [
          { lineId: 11, quantity: "10" },
          { lineId: 12, quantity: "10" },
        ],
      }),
    );
  });

  it("refuse plus de trois décimales avant l'enregistrement", async () => {
    vi.mocked(invoke).mockResolvedValue(detail());
    renderDetail();
    fireEvent.change(await screen.findByLabelText("Quantité comptée de Colle"), {
      target: { value: "1,2345" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Enregistrer les quantités" }));

    expect(await screen.findByText(/dépasse 3 décimales/)).toBeTruthy();
    expect(invoke).not.toHaveBeenCalledWith("save_inventory_counts", expect.anything());
  });

  it("empêche la validation d'un inventaire incomplet", async () => {
    vi.mocked(invoke).mockResolvedValue(detail());
    renderDetail();
    const button = await screen.findByRole("button", { name: "Valider l'inventaire" });
    expect((button as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByText("Produits comptés")).toBeTruthy();
    expect(screen.getByText("Sans écart")).toBeTruthy();
  });

  it("demande une confirmation avant de valider", async () => {
    const onStockChanged = vi.fn();
    const ready = detail({
      counts: { counted: 2, unchanged: 1, surplus: 0, shortage: 1, uncounted: 0, stale: 0 },
      lines: [
        {
          id: 11,
          productId: 1,
          productName: "Colle",
          productReference: "COL-1",
          typeName: "Autre",
          theoretical: "10",
          counted: "10",
          current: "10",
          variance: "0",
          drifted: false,
        },
      ],
    });
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "validate_inventory") {
        return { ...ready, status: "validated", validatedAt: "2026-03-02T10:00:00.000Z" };
      }
      return ready;
    });
    render(
      <InventoryDetail
        inventoryId={4}
        onBack={() => undefined}
        onOpenSheet={() => undefined}
        onStockChanged={onStockChanged}
      />,
    );
    fireEvent.click(await screen.findByRole("button", { name: "Valider l'inventaire" }));
    expect(screen.getByText(/Cette action est définitive/)).toBeTruthy();
    expect(invoke).not.toHaveBeenCalledWith("validate_inventory", expect.anything());
    fireEvent.click(screen.getByRole("button", { name: "Confirmer la validation" }));
    expect(await screen.findByText("Inventaire validé. Il n'est plus modifiable.")).toBeTruthy();
    expect(invoke).toHaveBeenCalledWith("validate_inventory", { id: 4 });
    expect(onStockChanged).toHaveBeenCalled();
  });

  it("bloque la validation tant qu'une quantité modifiée n'est pas enregistrée", async () => {
    const line = {
      id: 11,
      productId: 1,
      productName: "Colle",
      productReference: "COL-1",
      typeName: "Autre",
      theoretical: "10",
      counted: "80",
      current: "10",
      variance: "70",
      drifted: false,
    };
    const saved = detail({
      lines: [line],
      counts: { counted: 1, unchanged: 0, surplus: 1, shortage: 0, uncounted: 0, stale: 0 },
    });
    const corrected = detail({
      lines: [{ ...line, counted: "8", variance: "-2" }],
      counts: { counted: 1, unchanged: 0, surplus: 0, shortage: 1, uncounted: 0, stale: 0 },
    });
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "save_inventory_counts") {
        return corrected;
      }
      if (command === "validate_inventory") {
        return { ...corrected, status: "validated", validatedAt: "2026-03-02T10:00:00.000Z" };
      }
      return saved;
    });
    renderDetail();
    const button = (await screen.findByRole("button", { name: "Valider l'inventaire" })) as HTMLButtonElement;
    expect(button.disabled).toBe(false);

    // Correction de la saisie 80 -> 8 sans l'enregistrer : la base contient encore 80.
    fireEvent.change(screen.getByLabelText("Quantité comptée de Colle"), { target: { value: "8" } });
    expect(button.disabled).toBe(true);
    expect(screen.getByText(/1 quantité saisie n'est pas enregistrée\./)).toBeTruthy();

    // Une écriture équivalente (virgule, zéros) n'est pas une modification.
    fireEvent.change(screen.getByLabelText("Quantité comptée de Colle"), { target: { value: "80,000" } });
    expect(button.disabled).toBe(false);

    fireEvent.change(screen.getByLabelText("Quantité comptée de Colle"), { target: { value: "8" } });
    fireEvent.click(screen.getByRole("button", { name: "Enregistrer les quantités" }));
    expect(await screen.findByText("Manque de 2")).toBeTruthy();
    expect(button.disabled).toBe(false);
    fireEvent.click(button);
    fireEvent.click(screen.getByRole("button", { name: "Confirmer la validation" }));
    expect(await screen.findByText("Inventaire validé. Il n'est plus modifiable.")).toBeTruthy();
    expect(invoke).toHaveBeenCalledWith("validate_inventory", { id: 4 });
  });

  it("signale un stock modifié et propose de réactualiser", async () => {
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "refresh_inventory_line") {
        return detail({
          lines: [
            {
              id: 11,
              productId: 1,
              productName: "Colle",
              productReference: "COL-1",
              typeName: "Autre",
              theoretical: "12",
              counted: "10",
              current: "12",
              variance: "-2",
              drifted: false,
            },
          ],
          counts: { counted: 1, unchanged: 0, surplus: 0, shortage: 1, uncounted: 0, stale: 0 },
        });
      }
      return detail({
        lines: [
          {
            id: 11,
            productId: 1,
            productName: "Colle",
            productReference: "COL-1",
            typeName: "Autre",
            theoretical: "10",
            counted: "10",
            current: "12",
            variance: "0",
            drifted: true,
          },
        ],
        counts: { counted: 1, unchanged: 0, surplus: 0, shortage: 0, uncounted: 0, stale: 1 },
      });
    });
    renderDetail();
    expect(await screen.findByText("Référence périmée")).toBeTruthy();
    expect(screen.getByText(/passé de 10 à 12/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Réactualiser la référence" }));
    expect(await screen.findByText("Manque de 2")).toBeTruthy();
    expect(invoke).toHaveBeenCalledWith("refresh_inventory_line", { lineId: 11, resetCount: false });
  });

  it("laisse un inventaire validé en lecture seule", async () => {
    vi.mocked(invoke).mockResolvedValue(
      detail({
        status: "validated",
        validatedAt: "2026-03-02T10:00:00.000Z",
        counts: { counted: 2, unchanged: 1, surplus: 0, shortage: 1, uncounted: 0, stale: 0 },
      }),
    );
    renderDetail();
    const input = await screen.findByLabelText("Quantité comptée de Colle");
    expect((input as HTMLInputElement).disabled).toBe(true);
    expect(screen.getByText("Inventaire validé. Il n'est plus modifiable.")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Enregistrer les quantités" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Valider l'inventaire" })).toBeNull();
  });
});

