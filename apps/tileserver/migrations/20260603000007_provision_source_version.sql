-- Freshness tracking for provisioning. `source_version` is a content hash
-- of the restored N50 SQL dump for the current area; a scheduled or repeat
-- `provision-n50` downloads, hashes, and — when the hash is unchanged —
-- skips the expensive restore + upserts + matview refresh entirely. NULL
-- for rows written before this column existed (treated as "unknown", so the
-- next run always does the full provision once).
ALTER TABLE paths.provision_state ADD COLUMN IF NOT EXISTS source_version text;
