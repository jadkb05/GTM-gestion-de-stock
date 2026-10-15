-- Phase 5 du projet, version de schéma 4.
-- Une base déjà en version 3 reçoit ces tables et colonnes sans réécriture
-- des mouvements existants : source vaut 'manual' et inventory_id reste NULL.
-- Les produits existants n'ont pas de seuil (reorder_threshold_milli NULL).
-- La contrainte direction IN ('in','out') n'est pas modifiée. Une correction
-- d'inventaire est un mouvement in (surplus) ou out (manque), identifié par
-- source = 'inventory' et par inventory_id.

CREATE TABLE inventories (
    id INTEGER PRIMARY KEY,
    name TEXT,
    comment TEXT,
    status TEXT NOT NULL CHECK (status IN ('draft', 'validated')),
    created_at TEXT NOT NULL,
    validated_at TEXT,
    CHECK (
        (status = 'draft' AND validated_at IS NULL)
        OR (status = 'validated' AND validated_at IS NOT NULL)
    )
);

CREATE TABLE inventory_lines (
    id INTEGER PRIMARY KEY,
    inventory_id INTEGER NOT NULL REFERENCES inventories (id),
    product_id INTEGER NOT NULL REFERENCES products (id),
    theoretical_milli INTEGER NOT NULL CHECK (theoretical_milli >= 0),
    counted_milli INTEGER CHECK (counted_milli IS NULL OR counted_milli >= 0),
    snapshot_at TEXT NOT NULL,
    UNIQUE (inventory_id, product_id)
);

CREATE INDEX inventory_lines_inventory_idx ON inventory_lines (inventory_id);

ALTER TABLE products
    ADD COLUMN reorder_threshold_milli INTEGER
    CHECK (reorder_threshold_milli IS NULL OR reorder_threshold_milli >= 0);

ALTER TABLE stock_movements
    ADD COLUMN source TEXT NOT NULL DEFAULT 'manual'
    CHECK (source IN ('manual', 'inventory'));

ALTER TABLE stock_movements
    ADD COLUMN inventory_id INTEGER REFERENCES inventories (id);

ALTER TABLE stock_movements
    ADD COLUMN inventory_line_id INTEGER REFERENCES inventory_lines (id);

ALTER TABLE stock_movements
    ADD COLUMN variance_milli INTEGER;

CREATE INDEX stock_movements_inventory_idx ON stock_movements (inventory_id);

UPDATE app_metadata SET value = '4' WHERE key = 'schema_phase';
