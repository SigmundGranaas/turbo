-- recommend.*: tunables and caches owned by the recommendation
-- composition layer. The composition recipes are stateless; everything
-- they need to behave consistently across deployments lives here.

CREATE SCHEMA IF NOT EXISTS recommend;

-- Single-row table tracking the schema version of the derived edge
-- attributes (max_slope, landcover, exposure, ...). Bumped by ingest
-- migrations whenever the definition of a derived attribute changes.
-- The engine reads it at startup and refuses to serve if it doesn't
-- recognise the version. Embedded in every CandidateId so client-held
-- IDs invalidate cleanly when the data changes.
CREATE TABLE recommend.attr_version (
    singleton   boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    version     integer NOT NULL,
    notes       text,
    set_at      timestamptz NOT NULL DEFAULT now()
);
INSERT INTO recommend.attr_version (version, notes)
    VALUES (1, 'initial schema; no derived attrs populated yet');

-- Per-profile blend weights for the composition layer. Code ships
-- defaults; this table is the runtime source of truth and is loaded
-- into a process-resident cache at startup.
CREATE TABLE recommend.profile_weights (
    profile             text PRIMARY KEY,           -- hiking|ski|bike-gravel|bike-road
    terrain_quality     double precision NOT NULL,
    trail_quality       double precision NOT NULL,
    effort_match        double precision NOT NULL,
    variety             double precision NOT NULL,
    safety_exposure     double precision NOT NULL,
    anchor_payoff       double precision NOT NULL,
    updated_at          timestamptz NOT NULL DEFAULT now()
);

INSERT INTO recommend.profile_weights
    (profile,        terrain_quality, trail_quality, effort_match, variety, safety_exposure, anchor_payoff) VALUES
    ('hiking',       0.30,            0.20,          0.25,         0.10,    0.10,            0.05),
    ('ski',          0.25,            0.15,          0.25,         0.10,    0.20,            0.05),
    ('bike-gravel',  0.20,            0.30,          0.25,         0.10,    0.10,            0.05),
    ('bike-road',    0.10,            0.40,          0.30,         0.10,    0.05,            0.05);

-- Per-dimension calibration: 5th and 95th percentile of the raw
-- score distribution observed in training data, used by
-- calibration::normalise() to winsorise and min-max into [0,1].
-- One row per (profile, dimension). Code ships sane defaults; the
-- calibration job updates this from observed query telemetry.
CREATE TABLE recommend.scorer_calibration (
    profile             text NOT NULL,
    dimension           text NOT NULL,
    raw_p05             double precision NOT NULL,
    raw_p95             double precision NOT NULL,
    updated_at          timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (profile, dimension)
);

-- Difficulty grade thresholds. Aligned with DNT's 4-grade scale.
-- Code ships defaults; tunable in DB for region-specific calibration.
CREATE TABLE recommend.difficulty_grade (
    profile             text NOT NULL,
    grade               text NOT NULL,              -- gronn|bla|rod|svart
    max_distance_km     double precision NOT NULL,
    max_gain_m          double precision NOT NULL,
    max_slope_deg       double precision NOT NULL,
    seq                 integer NOT NULL,           -- ordering within profile
    PRIMARY KEY (profile, grade)
);

INSERT INTO recommend.difficulty_grade
    (profile,  grade,   max_distance_km, max_gain_m, max_slope_deg, seq) VALUES
    ('hiking', 'gronn',   5.0,             150.0,      15.0,         1),
    ('hiking', 'bla',    10.0,             400.0,      25.0,         2),
    ('hiking', 'rod',    20.0,             800.0,      35.0,         3),
    ('hiking', 'svart',  9999.0,           99999.0,    90.0,         4);
