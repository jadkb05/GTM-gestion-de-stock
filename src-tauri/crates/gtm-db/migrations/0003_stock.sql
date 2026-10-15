CREATE TABLE stock_balances (
    product_id INTEGER PRIMARY KEY REFERENCES products (id),
    quantity_milli INTEGER NOT NULL CHECK (quantity_milli >= 0),
    updated_at TEXT NOT NULL
);

CREATE TABLE stock_movements (
    id INTEGER PRIMARY KEY,
    product_id INTEGER NOT NULL REFERENCES products (id),
    direction TEXT NOT NULL CHECK (direction IN ('in', 'out')),
    quantity_milli INTEGER NOT NULL CHECK (quantity_milli > 0),
    reason TEXT,
    document_reference TEXT,
    comment TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX stock_movements_product_id ON stock_movements (product_id);
CREATE INDEX stock_movements_created_at ON stock_movements (created_at, id);

INSERT INTO stock_balances (product_id, quantity_milli, updated_at)
SELECT id, 0, updated_at FROM products;

CREATE TRIGGER products_create_stock_balance
AFTER INSERT ON products
BEGIN
    INSERT INTO stock_balances (product_id, quantity_milli, updated_at)
    VALUES (NEW.id, 0, NEW.created_at);
END;

UPDATE app_metadata SET value = '3' WHERE key = 'schema_phase';
