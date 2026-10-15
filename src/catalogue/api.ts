import { invoke } from "@tauri-apps/api/core";
import type { NamedRecord, Product, ProductInput, ProductQuery } from "./types";

type ProductPayload = {
  name: string;
  reference: string | null;
  typeId: number;
  supplierId: number | null;
  description: string | null;
};

function optionalId(value: string): number | null {
  if (value.trim().length === 0) {
    return null;
  }
  return Number(value);
}

function payload(input: ProductInput): ProductPayload {
  const reference = input.reference.trim();
  const description = input.description.trim();
  return {
    name: input.name.trim(),
    reference: reference.length === 0 ? null : reference,
    typeId: Number(input.typeId),
    supplierId: optionalId(input.supplierId),
    description: description.length === 0 ? null : description,
  };
}

export function listProducts(query: ProductQuery): Promise<Product[]> {
  return invoke<Product[]>("list_products", {
    query: {
      search: query.search.trim(),
      typeId: optionalId(query.typeId),
      supplierId: optionalId(query.supplierId),
      status: query.status,
    },
  });
}

export function createProduct(input: ProductInput): Promise<Product> {
  return invoke<Product>("create_product", { product: payload(input) });
}

export function updateProduct(id: number, input: ProductInput): Promise<Product> {
  return invoke<Product>("update_product", { id, product: payload(input) });
}

export function setProductActive(id: number, active: boolean): Promise<Product> {
  return invoke<Product>("set_product_active", { id, active });
}

export function listProductTypes(includeInactive: boolean): Promise<NamedRecord[]> {
  return invoke<NamedRecord[]>("list_product_types", { includeInactive });
}

export function createProductType(name: string): Promise<NamedRecord> {
  return invoke<NamedRecord>("create_product_type", { name: name.trim() });
}

export function updateProductType(id: number, name: string): Promise<NamedRecord> {
  return invoke<NamedRecord>("update_product_type", { id, name: name.trim() });
}

export function setProductTypeActive(id: number, active: boolean): Promise<NamedRecord> {
  return invoke<NamedRecord>("set_product_type_active", { id, active });
}

export function listSuppliers(includeInactive: boolean): Promise<NamedRecord[]> {
  return invoke<NamedRecord[]>("list_suppliers", { includeInactive });
}

export function createSupplier(name: string): Promise<NamedRecord> {
  return invoke<NamedRecord>("create_supplier", { name: name.trim() });
}

export function updateSupplier(id: number, name: string): Promise<NamedRecord> {
  return invoke<NamedRecord>("update_supplier", { id, name: name.trim() });
}

export function setSupplierActive(id: number, active: boolean): Promise<NamedRecord> {
  return invoke<NamedRecord>("set_supplier_active", { id, active });
}
