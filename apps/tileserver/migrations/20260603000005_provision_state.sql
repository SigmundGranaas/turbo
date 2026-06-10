-- Records which Geonorge area the canonical N50 tables were last provisioned
-- from. Every n50 upsert is `DELETE WHERE source='n50'` + INSERT, so a
-- county provision silently REPLACES a national one (wipes ~90% of
-- coverage). This singleton lets `provision-n50` refuse that footgun unless
-- forced, and lets operators see the current coverage at a glance.
CREATE TABLE paths.provision_state (
    singleton      boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    area           text NOT NULL,                 -- '0000' = national, else fylke code
    row_count      bigint NOT NULL DEFAULT 0,
    provisioned_at timestamptz NOT NULL DEFAULT now()
);
