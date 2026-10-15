CREATE TABLE app_metadata (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);

INSERT INTO app_metadata (key, value) VALUES ('app_name', 'GTM Stock Manager');
INSERT INTO app_metadata (key, value) VALUES ('schema_phase', '1');
