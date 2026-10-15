import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Product } from "../catalogue/types";
import { MovementForm } from "./MovementForm";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const mastic: Product = {
  id: 100,
  name: "Mastic façade",
  reference: "SIL-01",
  typeId: 1,
  typeName: "Silicone",
  supplierId: null,
  supplierName: null,
  description: null,
  isActive: true,
  createdAt: "2026-01-01T00:00:00Z",
  updatedAt: "2026-01-01T00:00:00Z",
};

const movement = {
  id: 7,
  productId: 100,
  productName: "Mastic façade",
  productReference: "SIL-01",
  direction: "in",
  quantity: "1.5",
  reason: "Réception",
  documentReference: "BL-10",
  comment: null,
  createdAt: "2026-10-13T12:00:00Z",
};

function mockStock(stock = "2", active = true) {
  vi.mocked(invoke).mockImplementation(async (command: string) => {
    if (command === "list_products") {
      return active ? [mastic] : [];
    }
    if (command === "get_product_stock") {
      return {
        productId: 100,
        productName: "Mastic façade",
        isActive: active,
        quantity: stock,
      };
    }
    if (command === "record_stock_entry" || command === "record_stock_exit") {
      return { ...movement, direction: command === "record_stock_entry" ? "in" : "out", quantity: "1" };
    }
    throw new Error(command);
  });
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

async function chooseProduct() {
  fireEvent.change(await screen.findByLabelText("Produit"), { target: { value: "100" } });
  expect(await screen.findByText("2")).toBeTruthy();
}

describe("mouvements de stock", () => {
  it("enregistre une entrée après récapitulatif", async () => {
    mockStock("2");
    render(<MovementForm direction="in" initialProductId={null} />);
    await chooseProduct();
    fireEvent.change(screen.getByLabelText("Quantité"), { target: { value: "1,5" } });
    fireEvent.change(screen.getByLabelText("Motif"), { target: { value: "Réception" } });
    fireEvent.click(screen.getByRole("button", { name: "Vérifier l'opération" }));
    expect(await screen.findByRole("heading", { name: "Récapitulatif" })).toBeTruthy();
    expect(screen.getByText("Entrée")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Enregistrer l'entrée" }));
    expect((await screen.findByRole("status")).textContent).toContain("Entrée enregistrée.");
    expect(invoke).toHaveBeenCalledWith("record_stock_entry", {
      movement: {
        productId: 100,
        quantity: "1.5",
        reason: "Réception",
        documentReference: null,
        comment: null,
      },
    });
  });

  it("poursuit une entrée de 100 lorsque le stock disponible est nul", async () => {
    mockStock("0");
    render(<MovementForm direction="in" initialProductId={null} />);
    fireEvent.change(await screen.findByLabelText("Produit"), { target: { value: "100" } });
    expect(await screen.findByText("0")).toBeTruthy();
    fireEvent.change(screen.getByLabelText("Quantité"), { target: { value: "100" } });
    fireEvent.change(screen.getByLabelText("Motif"), { target: { value: "lolol" } });
    fireEvent.change(screen.getByLabelText("Référence documentaire"), { target: { value: "llklsdks" } });
    fireEvent.click(screen.getByRole("button", { name: "Vérifier l'opération" }));

    expect(await screen.findByRole("heading", { name: "Récapitulatif" })).toBeTruthy();
    expect(screen.queryByRole("alert")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Enregistrer l'entrée" }));
    expect((await screen.findByRole("status")).textContent).toContain("Entrée enregistrée.");
    expect(invoke).toHaveBeenCalledWith("record_stock_entry", {
      movement: {
        productId: 100,
        quantity: "100",
        reason: "lolol",
        documentReference: "llklsdks",
        comment: null,
      },
    });
  });

  it("refuse une sortie supérieure au stock connu", async () => {
    mockStock("2");
    render(<MovementForm direction="out" initialProductId={100} />);
    expect(await screen.findByText("2")).toBeTruthy();
    fireEvent.change(screen.getByLabelText("Quantité"), { target: { value: "5" } });
    expect(await screen.findByRole("alert")).toBeTruthy();
    expect(screen.getByText("Le stock disponible est insuffisant.")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Vérifier l'opération" }));
    expect(screen.queryByRole("button", { name: "Enregistrer la sortie" })).toBeNull();
    expect(invoke).not.toHaveBeenCalledWith("record_stock_exit", expect.anything());
  });

  it("valide une quantité vide", async () => {
    mockStock("2");
    render(<MovementForm direction="in" initialProductId={100} />);
    expect(await screen.findByText("2")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Vérifier l'opération" }));
    expect(await screen.findByText("Indiquez une quantité.")).toBeTruthy();
  });

  it("affiche l'erreur renvoyée par le backend", async () => {
    mockStock("2");
    render(<MovementForm direction="out" initialProductId={100} />);
    expect(await screen.findByText("2")).toBeTruthy();
    fireEvent.change(screen.getByLabelText("Quantité"), { target: { value: "1" } });
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "record_stock_exit") {
        throw "Le stock disponible est insuffisant.";
      }
      if (command === "list_products") {
        return [mastic];
      }
      if (command === "get_product_stock") {
        return { productId: 100, productName: "Mastic façade", isActive: true, quantity: "2" };
      }
      return [];
    });
    fireEvent.click(screen.getByRole("button", { name: "Vérifier l'opération" }));
    fireEvent.click(await screen.findByRole("button", { name: "Enregistrer la sortie" }));
    expect(await screen.findByText("Le stock disponible est insuffisant.")).toBeTruthy();
  });

  it("empêche une double soumission", async () => {
    let calls = 0;
    let release: (value: unknown) => void = () => undefined;
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "list_products") {
        return [mastic];
      }
      if (command === "get_product_stock") {
        return { productId: 100, productName: "Mastic façade", isActive: true, quantity: "2" };
      }
      if (command === "record_stock_entry") {
        calls += 1;
        return new Promise((resolve) => {
          release = resolve;
        });
      }
      return [];
    });
    render(<MovementForm direction="in" initialProductId={100} />);
    expect(await screen.findByText("2")).toBeTruthy();
    fireEvent.change(screen.getByLabelText("Quantité"), { target: { value: "1" } });
    fireEvent.click(screen.getByRole("button", { name: "Vérifier l'opération" }));
    const save = await screen.findByRole("button", { name: "Enregistrer l'entrée" });
    fireEvent.click(save);
    fireEvent.click(save);
    expect(calls).toBe(1);
    release({ ...movement, quantity: "1" });
  });

  it("n'ouvre pas d'opération pour un produit désactivé", async () => {
    mockStock("4", false);
    render(<MovementForm direction="in" initialProductId={100} />);
    expect(await screen.findByText(/Ce produit est désactivé/)).toBeTruthy();
    expect(screen.getByText("4")).toBeTruthy();
    expect((screen.getByRole("button", { name: "Vérifier l'opération" }) as HTMLButtonElement).disabled).toBe(
      true,
    );
  });
});

describe("recherche du produit", () => {
  it("filtre la liste et choisit le seul produit correspondant", async () => {
    mockStock("2");
    render(<MovementForm direction="in" initialProductId={null} />);
    fireEvent.change(await screen.findByLabelText("Rechercher un produit"), { target: { value: "sil-0" } });
    expect(screen.getByText("1 produit correspond.")).toBeTruthy();
    expect((screen.getByLabelText("Produit") as HTMLSelectElement).value).toBe("100");
    expect(await screen.findByText("2")).toBeTruthy();
  });

  it("indique l'absence de correspondance", async () => {
    mockStock("2");
    render(<MovementForm direction="in" initialProductId={null} />);
    fireEvent.change(await screen.findByLabelText("Rechercher un produit"), { target: { value: "zzz" } });
    expect(screen.getByText("Aucun produit actif ne correspond.")).toBeTruthy();
  });
});
