import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { NamedRecord } from "../catalogue/types";
import { ProductTypesPage } from "./ProductTypesPage";
import { SuppliersPage } from "./SuppliersPage";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const silicone: NamedRecord = {
  id: 1,
  name: "Silicone",
  isActive: true,
  createdAt: "2026-01-01T00:00:00Z",
  updatedAt: "2026-01-01T00:00:00Z",
};

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("types et fournisseurs", () => {
  it("ajoute, modifie et désactive un type", async () => {
    const created: NamedRecord = { ...silicone, id: 4, name: "Mousse" };
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "list_product_types") {
        return [silicone];
      }
      if (command === "create_product_type") {
        return created;
      }
      if (command === "update_product_type") {
        return { ...created, name: "Mousse PU" };
      }
      if (command === "set_product_type_active") {
        return { ...silicone, isActive: false };
      }
      throw new Error(command);
    });

    render(<ProductTypesPage />);
    expect(await screen.findByText("Silicone")).toBeTruthy();
    expect(screen.getByText(/Vissable/)).toBeTruthy();

    fireEvent.change(screen.getByLabelText("Nom"), { target: { value: "Mousse" } });
    fireEvent.click(screen.getByRole("button", { name: "Ajouter" }));
    expect((await screen.findByRole("status")).textContent).toContain("Enregistrement effectué.");
    expect(invoke).toHaveBeenCalledWith("create_product_type", { name: "Mousse" });

    fireEvent.click(screen.getAllByRole("button", { name: "Modifier" })[0]);
    fireEvent.change(screen.getByLabelText("Nouveau nom"), { target: { value: "Mousse PU" } });
    fireEvent.click(screen.getByRole("button", { name: "Enregistrer" }));
    expect(await screen.findByText("Modification enregistrée.")).toBeTruthy();

    fireEvent.click(screen.getAllByRole("button", { name: "Désactiver" })[0]);
    expect(screen.getByText(/produits déjà associés restent en place/i)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Confirmer la désactivation" }));
    expect(await screen.findByText(/Entrée désactivée/)).toBeTruthy();
    expect(invoke).toHaveBeenCalledWith("set_product_type_active", { id: 1, active: false });
  });

  it("refuse un nom de fournisseur vide et signale un doublon", async () => {
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "list_suppliers") {
        return [{ ...silicone, id: 10, name: "Selena" }];
      }
      if (command === "create_supplier") {
        throw "Un fournisseur portant ce nom existe déjà.";
      }
      throw new Error(command);
    });

    render(<SuppliersPage />);
    expect(await screen.findByText("Selena")).toBeTruthy();
    expect(screen.getByText(/Tenois/)).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Ajouter" }));
    expect(await screen.findByText("Indiquez le nom du fournisseur.")).toBeTruthy();

    fireEvent.change(screen.getByLabelText("Nom"), { target: { value: "Selena" } });
    fireEvent.click(screen.getByRole("button", { name: "Ajouter" }));
    expect(await screen.findByText("Un fournisseur portant ce nom existe déjà.")).toBeTruthy();
  });
});
