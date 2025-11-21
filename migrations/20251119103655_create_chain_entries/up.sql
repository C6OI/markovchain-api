CREATE TABLE chain_entries
(
    id     SERIAL PRIMARY KEY,
    "from" VARCHAR NOT NULL,
    "to"   VARCHAR NOT NULL,
    count  INTEGER NOT NULL,
    UNIQUE ("from", "to")
);
