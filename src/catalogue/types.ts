export type NamedRecord = {
  id: number;
  name: string;
  isActive: boolean;
  createdAt: string;
  updatedAt: string;
};

export type Product = {
  id: number;
  name: string;
  reference: string | null;
  typeId: number;
  typeName: string;
  supplierId: number | null;
  supplierName: string | null;
  description: string | null;
  isActive: boolean;
  createdAt: string;
  updatedAt: string;
};

export type ProductStatusFilter = "active" | "inactive" | "all";

export type ProductQuery = {
  search: string;
  typeId: string;
  supplierId: string;
  status: ProductStatusFilter;
};

export type ProductInput = {
  name: string;
  reference: string;
  typeId: string;
  supplierId: string;
  description: string;
};

export const emptyProductInput: ProductInput = {
  name: "",
  reference: "",
  typeId: "",
  supplierId: "",
  description: "",
};
