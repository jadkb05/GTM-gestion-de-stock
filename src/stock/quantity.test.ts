import { describe, expect, it } from "vitest";
import { formatQuantity, parseNonNegative, parseQuantity } from "./quantity";

describe("quantités de stock", () => {
  it("accepte une décimale avec virgule ou point", () => {
    expect(parseQuantity("1,5")).toEqual({ milli: 1500 });
    expect(parseQuantity(" 0.001 ")).toEqual({ milli: 1 });
    expect(formatQuantity(1500)).toBe("1.5");
    expect(formatQuantity(1)).toBe("0.001");
  });

  it("refuse zéro, négatif et valeur invalide", () => {
    expect(parseQuantity("0")).toEqual({
      error: "La quantité doit être strictement positive.",
    });
    expect(parseQuantity("-1")).toEqual({
      error: "La quantité doit être strictement positive.",
    });
    expect(parseQuantity("abc")).toEqual({ error: "La quantité n'est pas un nombre valide." });
    expect(parseQuantity("1.2345")).toEqual({ error: "La quantité dépasse 3 décimales." });
  });

  it("accepte zéro pour un comptage ou un seuil", () => {
    expect(parseNonNegative("0")).toEqual({ milli: 0 });
    expect(parseNonNegative("1,5")).toEqual({ milli: 1500 });
    expect(parseNonNegative("-1")).toEqual({ error: "La quantité doit être positive ou nulle." });
    expect(parseNonNegative("1.2345")).toEqual({ error: "La quantité dépasse 3 décimales." });
  });
});
