import { describe, expect, it } from "vitest";
import { validateLabel, validateProduct } from "./validation";

describe("validation du catalogue", () => {
  it("exige un nom et un type", () => {
    const errors = validateProduct({
      name: "  ",
      reference: "",
      typeId: "",
      supplierId: "",
      description: "",
    });
    expect(errors.name).toBe("Indiquez le nom du produit.");
    expect(errors.typeId).toBe("Choisissez un type de produit.");
  });

  it("accepte une référence vide", () => {
    expect(
      validateProduct({
        name: "Mastic",
        reference: "   ",
        typeId: "1",
        supplierId: "",
        description: "",
      }),
    ).toEqual({});
  });

  it("refuse un libellé vide", () => {
    expect(validateLabel("  ", "Indiquez le nom du type.")).toBe("Indiquez le nom du type.");
  });
});
