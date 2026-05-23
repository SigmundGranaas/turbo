-- Ingest job log. Every `tileserver ingest --job X` run writes one row;
-- the admin Jobs screen polls this table.

CREATE TYPE paths.job_status AS ENUM ('queued', 'running', 'succeeded', 'failed');

CREATE TABLE paths.ingest_job (
    id              bigserial PRIMARY KEY,
    run_id          uuid NOT NULL,
    name            text NOT NULL,
    status          paths.job_status NOT NULL DEFAULT 'queued',
    started_at      timestamptz,
    finished_at     timestamptz,
    rows_in         bigint NOT NULL DEFAULT 0,
    rows_upserted   bigint NOT NULL DEFAULT 0,
    error_text      text,
    log             jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at      timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX ingest_job_name_started_idx
    ON paths.ingest_job (name, started_at DESC);
CREATE INDEX ingest_job_status_idx
    ON paths.ingest_job (status) WHERE status IN ('queued', 'running');
