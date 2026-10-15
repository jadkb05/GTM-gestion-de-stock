import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ImportArticles } from "./ImportArticles";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  save: vi.fn(),
  open: vi.fn(),
}));

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("import d'articles", () => {
  it("n'enregistre rien si l'aperçu est annulé ou contient une erreur", async () => {
    vi.mocked(open).mockResolvedValue("/tmp/articles.csv");
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "preview_article_import") {
        return {
          ready: [],
          issues: [{ line: 2, message: "Le type « Inconnu » n'existe pas. Il ne sera pas créé." }],
          ignoredColumns: [],
        };
      }
      return { created: 1 };
    });
    const onImported = vi.fn();
    render(<ImportArticles onImported={onImported} />);

    fireEvent.click(screen.getByRole("button", { name: "Importer" }));
    expect(await screen.findByText(/n'existe pas/)).toBeTruthy();
    expect(screen.getByRole("button", { name: "Confirmer l'import" }).hasAttribute("disabled")).toBe(true);

    fireEvent.click(screen.getByRole("button", { name: "Annuler" }));
    expect(screen.queryByText(/n'existe pas/)).toBeNull();
    expect(invoke).not.toHaveBeenCalledWith("commit_article_import", expect.anything());
    expect(onImported).not.toHaveBeenCalled();
  });

  it("confirme un aperçu valide sans présenter le stock comme importé", async () => {
    vi.mocked(open).mockResolvedValue("/tmp/articles.csv");
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "preview_article_import") {
        return {
          ready: [{ line: 2, name: "Étanchéité", reference: "ET-1", typeName: "Silicone", supplierName: "Selena", description: null }],
          issues: [],
          ignoredColumns: [],
        };
      }
      if (command === "commit_article_import") {
        return { created: 1 };
      }
      return null;
    });
    const onImported = vi.fn();
    render(<ImportArticles onImported={onImported} />);
    fireEvent.click(screen.getByRole("button", { name: "Importer" }));
    expect(await screen.findByText("Étanchéité")).toBeTruthy();
    expect(screen.getByText(/Le stock reste inchangé/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Confirmer l'import" }));
    expect(await screen.findByRole("status")).toBeTruthy();
    expect(screen.getByRole("status").textContent).toContain("Le stock n'a pas été modifié.");
    expect(invoke).toHaveBeenCalledWith("commit_article_import", { path: "/tmp/articles.csv" });
    expect(onImported).toHaveBeenCalled();
  });
});