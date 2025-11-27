CREATE INDEX from_idx ON chain_entries ("from");

DROP INDEX idx_chain_entries_from_normalized;

ALTER TABLE chain_entries
    DROP COLUMN from_normalized;
