CREATE TABLE product_types (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE UNIQUE INDEX product_types_name_nocase ON product_types (name COLLATE NOCASE);

CREATE TABLE suppliers (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE UNIQUE INDEX suppliers_name_nocase ON suppliers (name COLLATE NOCASE);

CREATE TABLE products (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    reference TEXT,
    type_id INTEGER NOT NULL REFERENCES product_types (id),
    supplier_id INTEGER REFERENCES suppliers (id),
    description TEXT,
    is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE UNIQUE INDEX products_reference_nocase
    ON products (reference COLLATE NOCASE)
    WHERE reference IS NOT NULL;

CREATE INDEX products_type_id ON products (type_id);
CREATE INDEX products_supplier_id ON products (supplier_id);

INSERT INTO product_types (name, is_active, created_at, updated_at)
SELECT 'Silicone', 1, ts, ts FROM (SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now') AS ts);
INSERT INTO product_types (name, is_active, created_at, updated_at)
SELECT 'Acrylique', 1, ts, ts FROM (SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now') AS ts);
INSERT INTO product_types (name, is_active, created_at, updated_at)
SELECT 'Autre', 1, ts, ts FROM (SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now') AS ts);

INSERT INTO suppliers (name, is_active, created_at, updated_at)
SELECT 'Selena', 1, ts, ts FROM (SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now') AS ts);
INSERT INTO suppliers (name, is_active, created_at, updated_at)
SELECT 'Tremco', 1, ts, ts FROM (SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now') AS ts);
INSERT INTO suppliers (name, is_active, created_at, updated_at)
SELECT 'Legallais', 1, ts, ts FROM (SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now') AS ts);

UPDATE app_metadata SET value = '2' WHERE key = 'schema_phase';
