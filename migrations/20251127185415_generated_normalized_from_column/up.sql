ALTER TABLE chain_entries
    ADD COLUMN from_normalized TEXT
        GENERATED ALWAYS AS (LOWER(REGEXP_REPLACE("from", '\W', '', 'g'))) STORED;

CREATE INDEX idx_chain_entries_from_normalized ON chain_entries (from_normalized);

DROP INDEX from_idx;
