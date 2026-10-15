import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { NamedRecord, Product } from "../catalogue/types";
import { CATALOGUE_PAGE_SIZE, ProductCatalogue } from "./ProductCatalogue";

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

const selena: NamedRecord = {
  id: 10,
  name: "Selena",
  isActive: true,
  createdAt: "2026-01-01T00:00:00Z",
  updatedAt: "2026-01-01T00:00:00Z",
};

const mastic: Product = {
  id: 100,
  name: "Mastic façade",
  reference: "SIL-01",
  typeId: 1,
  typeName: "Silicone",
  supplierId: 10,
  supplierName: "Selena",
  description: null,
  isActive: true,
  createdAt: "2026-01-01T00:00:00Z",
  updatedAt: "2026-01-01T00:00:00Z",
};

function mockCatalogue(products: Product[]) {
  vi.mocked(invoke).mockImplementation(async (command: string) => {
    if (command === "list_product_types") {
      return [silicone];
    }
    if (command === "list_suppliers") {
      return [selena];
    }
    if (command === "list_products") {
      return products;
    }
    if (command === "list_stock_levels") {
      return products.map((product) => ({ productId: product.id, quantity: "3" }));
    }
    if (command === "set_product_active") {
      return { ...mastic, isActive: false };
    }
    throw new Error(`commande inattendue : ${command}`);
  });
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("catalogue des produits", () => {
  it("affiche les produits chargés", async () => {
    mockCatalogue([mastic]);
    render(<ProductCatalogue />);

    expect(await screen.findByRole("heading", { name: "Catalogue des produits" })).toBeTruthy();
    expect(screen.getByText("Mastic façade")).toBeTruthy();
    expect(screen.getByText("SIL-01")).toBeTruthy();
    const row = screen.getByText("Mastic façade").closest("tr");
    expect(row?.textContent).toContain("Silicone");
    expect(row?.textContent).toContain("Selena");
    expect(row?.textContent).toContain("Actif");
    expect(row?.textContent).toContain("3");
  });

  it("indique l'absence de produit", async () => {
    mockCatalogue([]);
    render(<ProductCatalogue />);
    expect(await screen.findByText("Aucun produit n'est enregistré.")).toBeTruthy();
  });

  it("transmet la recherche et les filtres", async () => {
    mockCatalogue([]);
    render(<ProductCatalogue />);
    await screen.findByText("Aucun produit n'est enregistré.");

    fireEvent.change(screen.getByLabelText("Recherche"), { target: { value: "sil" } });
    fireEvent.change(screen.getByLabelText("Type"), { target: { value: "1" } });
    fireEvent.change(screen.getByLabelText("Fournisseur"), { target: { value: "10" } });
    fireEvent.change(screen.getByLabelText("Statut"), { target: { value: "inactive" } });

    await screen.findByText("Aucun produit ne correspond à ces critères.");
    // La recherche part après une courte pause de frappe.
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("list_products", {
        query: { search: "sil", typeId: 1, supplierId: 10, status: "inactive" },
      });
    });
  });

  it("valide le formulaire avant l'enregistrement", async () => {
    mockCatalogue([]);
    render(<ProductCatalogue />);
    fireEvent.click(await screen.findByRole("button", { name: "Nouveau produit" }));
    fireEvent.click(screen.getByRole("button", { name: "Enregistrer" }));

    expect(await screen.findByText("Indiquez le nom du produit.")).toBeTruthy();
    expect(screen.getByText("Choisissez un type de produit.")).toBeTruthy();
    expect(invoke).not.toHaveBeenCalledWith("create_product", expect.anything());
  });

  it("affiche l'erreur renvoyée par le backend", async () => {
    mockCatalogue([]);
    render(<ProductCatalogue />);
    fireEvent.click(await screen.findByRole("button", { name: "Nouveau produit" }));
    fireEvent.change(screen.getByLabelText("Nom du produit"), { target: { value: "Mastic" } });
    fireEvent.change(screen.getByLabelText("Type de produit"), { target: { value: "1" } });
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "create_product") {
        throw "Cette référence est déjà utilisée.";
      }
      if (command === "list_product_types") {
        return [silicone];
      }
      if (command === "list_suppliers") {
        return [selena];
      }
      return [];
    });

    fireEvent.click(screen.getByRole("button", { name: "Enregistrer" }));
    expect((await screen.findByRole("alert")).textContent).toContain(
      "Cette référence est déjà utilisée.",
    );
  });

  it("crée puis modifie un produit", async () => {
    mockCatalogue([]);
    const created = { ...mastic, name: "Nouveau mastic" };
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "list_product_types") {
        return [silicone];
      }
      if (command === "list_suppliers") {
        return [selena];
      }
      if (command === "create_product") {
        return created;
      }
      if (command === "update_product") {
        return { ...created, name: "Mastic modifié" };
      }
      return [created];
    });

    render(<ProductCatalogue />);
    fireEvent.click(await screen.findByRole("button", { name: "Nouveau produit" }));
    fireEvent.change(screen.getByLabelText("Nom du produit"), { target: { value: "Nouveau mastic" } });
    fireEvent.change(screen.getByLabelText("Référence"), { target: { value: "SIL-01" } });
    fireEvent.change(screen.getByLabelText("Type de produit"), { target: { value: "1" } });
    fireEvent.click(screen.getByRole("button", { name: "Enregistrer" }));

    expect((await screen.findByRole("status")).textContent).toContain(
      "Produit enregistré : Nouveau mastic.",
    );
    expect(invoke).toHaveBeenCalledWith("create_product", {
      product: {
        name: "Nouveau mastic",
        reference: "SIL-01",
        typeId: 1,
        supplierId: null,
        description: null,
      },
    });

    fireEvent.click(await screen.findByRole("button", { name: "Modifier" }));
    fireEvent.change(screen.getByLabelText("Nom du produit"), { target: { value: "Mastic modifié" } });
    fireEvent.click(screen.getByRole("button", { name: "Enregistrer" }));
    expect((await screen.findByRole("status")).textContent).toContain(
      "Produit enregistré : Mastic modifié.",
    );
  });

  it("désactive un produit après confirmation", async () => {
    mockCatalogue([mastic]);
    render(<ProductCatalogue />);
    fireEvent.click(await screen.findByRole("button", { name: "Désactiver" }));
    fireEvent.click(screen.getByRole("button", { name: "Confirmer" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("set_product_active", { id: 100, active: false });
    });
  });

  it("affiche le stock d'un produit désactivé sans proposer d'opération", async () => {
    mockCatalogue([{ ...mastic, isActive: false }]);
    const onSheet = vi.fn();
    render(<ProductCatalogue onEntry={() => undefined} onExit={() => undefined} onOpenSheet={onSheet} />);
    const row = (await screen.findByText("Mastic façade")).closest("tr");
    expect(row?.textContent).toContain("Désactivé");
    expect(row?.textContent).toContain("3");
    expect(screen.queryByRole("button", { name: "Entrée" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Sortie" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Fiche" }));
    expect(onSheet).toHaveBeenCalledWith(100);
  });

  it("affiche l'erreur de chargement", async () => {
    vi.mocked(invoke).mockRejectedValue("La base locale n'a pas pu être ouverte.");
    render(<ProductCatalogue />);
    expect((await screen.findByRole("alert")).textContent).toContain(
      "La base locale n'a pas pu être ouverte.",
    );
  });
  it("affiche un grand catalogue par tranches", async () => {
    const many = Array.from({ length: 230 }, (_, index) => ({
      ...mastic,
      id: index + 1,
      name: `Article ${String(index).padStart(3, "0")}`,
      reference: `GEN-${index}`,
    }));
    mockCatalogue(many);
    render(<ProductCatalogue />);

    await screen.findByText("Article 000");
    const rows = () => document.querySelectorAll("tbody tr").length;
    expect(CATALOGUE_PAGE_SIZE).toBe(200);
    expect(rows()).toBe(200);
    expect(screen.getByText(/200 sur 230 produits affichés/)).toBeTruthy();
    expect(screen.queryByText("Article 200")).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Afficher les 30 suivants" }));
    expect(rows()).toBe(230);
    expect(screen.getByText("Article 229")).toBeTruthy();
    expect(screen.getByText("230 produits affichés")).toBeTruthy();
    expect(screen.queryByRole("button", { name: /Afficher les/ })).toBeNull();
  });

  it("revient à la première tranche quand la recherche change", async () => {
    const many = Array.from({ length: 230 }, (_, index) => ({ ...mastic, id: index + 1, name: `Article ${index}` }));
    mockCatalogue(many);
    render(<ProductCatalogue />);
    fireEvent.click(await screen.findByRole("button", { name: "Afficher les 30 suivants" }));
    expect(document.querySelectorAll("tbody tr").length).toBe(230);

    fireEvent.change(screen.getByLabelText("Recherche"), { target: { value: "Article" } });
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("list_products", expect.objectContaining({ query: expect.objectContaining({ search: "Article" }) }));
    });
    await waitFor(() => expect(document.querySelectorAll("tbody tr").length).toBe(200));
  });

  it("interroge la base une seule fois quand la frappe s'arrête", async () => {
    mockCatalogue([mastic]);
    render(<ProductCatalogue />);
    await screen.findByText("Mastic façade");
    vi.mocked(invoke).mockClear();

    const field = screen.getByLabelText("Recherche");
    fireEvent.change(field, { target: { value: "s" } });
    fireEvent.change(field, { target: { value: "si" } });
    fireEvent.change(field, { target: { value: "sil" } });

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("list_products", expect.objectContaining({ query: expect.objectContaining({ search: "sil" }) }));
    });
    const searches = vi
      .mocked(invoke)
      .mock.calls.filter(([command]) => command === "list_products")
      .map(([, args]) => (args as { query: { search: string } }).query.search);
    expect(searches).toEqual(["sil"]);
  });

  it("garde la liste affichée pendant un rechargement", async () => {
    mockCatalogue([mastic]);
    render(<ProductCatalogue />);
    await screen.findByText("Mastic façade");

    vi.mocked(invoke).mockImplementation(() => new Promise(() => undefined));
    fireEvent.change(screen.getByLabelText("Statut"), { target: { value: "all" } });

    await waitFor(() => expect(document.querySelector(".table-frame")?.getAttribute("aria-busy")).toBe("true"));
    expect(screen.getByText("Mastic façade")).toBeTruthy();
    expect(screen.queryByText("Chargement du catalogue…")).toBeNull();
  });
});
