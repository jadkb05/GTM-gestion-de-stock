import { describe, expect, it } from "vitest";
import { toErrorMessage } from "./api";

describe("toErrorMessage", () => {
  it("conserve un message textuel", () => {
    expect(toErrorMessage("Impossible d'ouvrir la base locale : disque plein")).toBe(
      "Impossible d'ouvrir la base locale : disque plein",
    );
  });

  it("lit le message d'une erreur", () => {
    expect(toErrorMessage(new Error("répertoire refusé"))).toBe("répertoire refusé");
  });

  it("fournit un message lorsque l'erreur est vide", () => {
    expect(toErrorMessage("")).toBe("La base locale n'a pas pu être initialisée.");
    expect(toErrorMessage(null)).toBe("La base locale n'a pas pu être initialisée.");
  });
});
