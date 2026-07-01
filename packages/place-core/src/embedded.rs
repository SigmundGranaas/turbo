//! The on-device engine: reads a per-region SQLite bundle and answers
//! reverse-geocode / search by funnelling candidates into the *same*
//! [`reverse_geocode`](crate::reverse_geocode) and
//! [`forward_search`](crate::forward_search) the server runs — so an offline
//! answer equals the online one by construction.
//!
//! Compiled with `--features embedded`. The bundle is a SQLite file:
//!
//! ```text
//! manifest(key, value)        -- format_version, ruleset_version, dataset_version, …
//! ruleset(json)               -- the exact ruleset artifact
//! places(id, name, name_fold, kind, lat, lng, status, elevation_m, kommune, fylke)
//! places_rtree(id,minLat,maxLat,minLng,maxLng)   -- R*Tree over place points
//! areas(id, area_type, name, kind, rings_json)   -- polygon rings [[[lng,lat],…],…]
//! areas_rtree(id,minLat,maxLat,minLng,maxLng)    -- bbox prefilter for containment
//! ```

use rusqlite::{Connection, OpenFlags};

use crate::geo::{haversine_m, point_in_polygon};
use crate::model::{Candidate, Kommune, LocationDescription, ProtectedArea, ReverseInput};
use crate::ruleset::Ruleset;
use crate::{reverse_geocode, SearchCandidate, SearchHit};

const REVERSE_RADIUS_M: f64 = 1000.0;
const REVERSE_LIMIT: usize = 25;

/// An opened region bundle.
pub struct Bundle {
    conn: Connection,
    ruleset: Ruleset,
}

#[derive(Debug)]
pub enum BundleError {
    Sqlite(rusqlite::Error),
    Ruleset(String),
}

impl From<rusqlite::Error> for BundleError {
    fn from(e: rusqlite::Error) -> Self {
        BundleError::Sqlite(e)
    }
}

impl Bundle {
    /// Open a bundle read-only and load its ruleset.
    pub fn open(path: &str) -> Result<Bundle, BundleError> {
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        let json: String = conn.query_row("SELECT json FROM ruleset LIMIT 1", [], |r| r.get(0))?;
        let ruleset = Ruleset::from_json(&json).map_err(|e| BundleError::Ruleset(e.to_string()))?;
        Ok(Bundle { conn, ruleset })
    }

    /// Reverse-geocode a coordinate from the bundle alone — mirroring the
    /// server's cascade exactly: kommune from polygon containment (falling back
    /// to the nearest feature's stored kommune), protected-area containment,
    /// and elevation from the nearest feature.
    pub fn reverse(&self, lat: f64, lng: f64) -> Result<Option<LocationDescription>, BundleError> {
        let toponyms = self.nearest(lat, lng, REVERSE_RADIUS_M, REVERSE_LIMIT)?;

        let nearest = toponyms.first();
        let elevation_m = nearest.and_then(|n| n.elevation_m);
        let nearest_kommune = nearest.and_then(|n| n.kommune.clone());
        let nearest_fylke = nearest.and_then(|n| n.fylke.clone());

        let kommune = match self.containing(lat, lng, "kommune")? {
            Some((name, fylke)) => Some(Kommune { name, fylke }),
            None => nearest_kommune.map(|name| Kommune {
                name,
                fylke: nearest_fylke,
            }),
        };
        let protected_area = self
            .containing(lat, lng, "protected_area")?
            .map(|(name, kind)| ProtectedArea { name, kind });

        let input = ReverseInput {
            toponyms: toponyms.into_iter().map(|t| t.candidate).collect(),
            protected_area,
            address: None,
            kommune,
            elevation_m,
        };
        Ok(reverse_geocode(&self.ruleset, &input))
    }

    /// Forward-search the bundle: trigram-free prefix/substring retrieval here
    /// (FTS5 joins once bundles carry it), final ordering by `forward_search`.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>, BundleError> {
        let fold = query.trim().to_lowercase();
        if fold.is_empty() {
            return Ok(Vec::new());
        }
        let like = format!("%{}%", fold.replace('%', "\\%").replace('_', "\\_"));
        let mut stmt = self.conn.prepare(
            "SELECT name, kind, kommune, fylke FROM places \
             WHERE name_fold LIKE ?1 ESCAPE '\\' LIMIT 200",
        )?;
        let rows = stmt.query_map([&like], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
            ))
        })?;

        // Pass raw kommune/fylke through; `forward_search` composes the subtitle
        // (human label + trimmed fylke) so online and offline format identically.
        let candidates: Vec<SearchCandidate> = rows
            .filter_map(Result::ok)
            .map(|(name, kind, kommune, fylke)| SearchCandidate {
                name,
                kind,
                distance_m: None,
                kommune,
                fylke,
                description: None,
            })
            .collect();

        let mut hits = crate::forward_search(&self.ruleset, query, &candidates);
        hits.truncate(limit);
        Ok(hits)
    }

    /// Nearest places within `radius_m`, closest first — R*Tree bbox prefilter
    /// then exact haversine.
    fn nearest(
        &self,
        lat: f64,
        lng: f64,
        radius_m: f64,
        limit: usize,
    ) -> Result<Vec<Nearby>, BundleError> {
        let dlat = radius_m / 111_320.0;
        let dlng = radius_m / (111_320.0 * lat.to_radians().cos().abs().max(1e-9));

        let mut stmt = self.conn.prepare(
            "SELECT p.name, p.kind, p.lat, p.lng, p.status, p.elevation_m, p.kommune, p.fylke \
             FROM places_rtree r JOIN places p ON p.id = r.id \
             WHERE r.maxLat >= ?1 AND r.minLat <= ?2 AND r.maxLng >= ?3 AND r.minLng <= ?4",
        )?;
        let rows = stmt.query_map([lat - dlat, lat + dlat, lng - dlng, lng + dlng], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, f64>(2)?,
                r.get::<_, f64>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, Option<f64>>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, Option<String>>(7)?,
            ))
        })?;

        let mut out: Vec<Nearby> = rows
            .filter_map(Result::ok)
            .filter_map(|(name, kind, plat, plng, status, elev, kommune, fylke)| {
                let distance = haversine_m(lat, lng, plat, plng);
                if distance > radius_m {
                    return None;
                }
                Some(Nearby {
                    candidate: Candidate {
                        name,
                        kind,
                        distance_m: distance,
                        status: Some(status),
                        secondary: None,
                    },
                    elevation_m: elev,
                    kommune,
                    fylke,
                })
            })
            .collect();

        out.sort_by(|a, b| a.candidate.distance_m.total_cmp(&b.candidate.distance_m));
        out.truncate(limit);
        Ok(out)
    }

    /// The smallest polygon of `area_type` containing the point — R*Tree bbox
    /// prefilter then exact point-in-polygon. Returns (name, kind): the
    /// protected-area's verneform, or the kommune's fylke.
    fn containing(
        &self,
        lat: f64,
        lng: f64,
        area_type: &str,
    ) -> Result<Option<(String, Option<String>)>, BundleError> {
        let mut stmt = self.conn.prepare(
            "SELECT a.name, a.kind, a.rings_json FROM areas_rtree r JOIN areas a ON a.id = r.id \
             WHERE a.area_type = ?1 \
             AND r.maxLat >= ?2 AND r.minLat <= ?2 AND r.maxLng >= ?3 AND r.minLng <= ?3",
        )?;
        let rows = stmt.query_map(rusqlite::params![area_type, lat, lng], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;

        let mut best: Option<(f64, (String, Option<String>))> = None;
        for (name, kind, rings_json) in rows.filter_map(Result::ok) {
            let Ok(rings) = serde_json::from_str::<Vec<Vec<(f64, f64)>>>(&rings_json) else {
                continue;
            };
            if !point_in_polygon(lng, lat, &rings) {
                continue;
            }
            // Crude area proxy (outer-ring bbox) to pick the smallest container.
            let area = ring_bbox_area(rings.first());
            if best.as_ref().is_none_or(|(a, _)| area < *a) {
                best = Some((area, (name, kind)));
            }
        }
        Ok(best.map(|(_, v)| v))
    }
}

struct Nearby {
    candidate: Candidate,
    elevation_m: Option<f64>,
    kommune: Option<String>,
    fylke: Option<String>,
}

fn ring_bbox_area(ring: Option<&Vec<(f64, f64)>>) -> f64 {
    match ring {
        None => f64::MAX,
        Some(r) => {
            let (mut min_x, mut min_y, mut max_x, mut max_y) =
                (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
            for &(x, y) in r {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
            (max_x - min_x) * (max_y - min_y)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Qualifier;

    /// Builds a tiny bundle: Galdhøpiggen (peak) + a Jotunheimen park square
    /// around a wilderness point. The embedded engine must reuse the shared
    /// ranking, so the answers equal the server's by construction.
    fn build_fixture() -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir()
            .join(format!(
                "place-core-bundle-{}-{}.sqlite",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::Relaxed)
            ))
            .to_string_lossy()
            .into_owned();
        let _ = std::fs::remove_file(&path);

        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE ruleset(json TEXT);
             CREATE TABLE places(id INTEGER PRIMARY KEY, name TEXT, name_fold TEXT, kind TEXT,
                 lat REAL, lng REAL, status TEXT, elevation_m REAL, kommune TEXT, fylke TEXT);
             CREATE VIRTUAL TABLE places_rtree USING rtree(id, minLat, maxLat, minLng, maxLng);
             CREATE TABLE areas(id INTEGER PRIMARY KEY, area_type TEXT, name TEXT, kind TEXT, rings_json TEXT);
             CREATE VIRTUAL TABLE areas_rtree USING rtree(id, minLat, maxLat, minLng, maxLng);",
        )
        .unwrap();

        conn.execute(
            "INSERT INTO ruleset(json) VALUES (?1)",
            [include_str!("../ruleset.v1.json")],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO places(id,name,name_fold,kind,lat,lng,status,elevation_m,kommune,fylke)
             VALUES (1,'Galdhøpiggen','galdhøpiggen','Fjell',61.63644,8.31248,'aktiv',2469.0,'Lom','Innlandet')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO places_rtree(id,minLat,maxLat,minLng,maxLng) VALUES (1,61.63644,61.63644,8.31248,8.31248)",
            [],
        )
        .unwrap();

        // Park square covering the wilderness point (61.50, 8.41).
        let rings = "[[[8.35,61.47],[8.47,61.47],[8.47,61.53],[8.35,61.53],[8.35,61.47]]]";
        conn.execute(
            "INSERT INTO areas(id,area_type,name,kind,rings_json) VALUES (1,'protected_area','Jotunheimen','Nasjonalpark',?1)",
            [rings],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO areas_rtree(id,minLat,maxLat,minLng,maxLng) VALUES (1,61.47,61.53,8.35,8.47)",
            [],
        )
        .unwrap();

        path
    }

    #[test]
    fn reverse_on_a_peak_uses_the_shared_ranking_with_enrichment() {
        let bundle = Bundle::open(&build_fixture()).unwrap();
        let d = bundle.reverse(61.6363, 8.3120).unwrap().expect("a result");

        assert_eq!(d.title, "Galdhøpiggen");
        assert_eq!(d.qualifier, Some(Qualifier::On));
        assert_eq!(d.kommune.as_deref(), Some("Lom"));
        assert_eq!(d.fylke.as_deref(), Some("Innlandet"));
        assert_eq!(d.elevation_m, Some(2469.0));
        assert!(d.distance_m.unwrap() < 100.0);
    }

    #[test]
    fn reverse_in_wilderness_falls_back_to_polygon_containment() {
        let bundle = Bundle::open(&build_fixture()).unwrap();
        // No place within 1 km; the park polygon must win.
        let d = bundle.reverse(61.50, 8.41).unwrap().expect("a result");

        assert_eq!(d.title, "Jotunheimen");
        assert_eq!(d.qualifier, Some(Qualifier::InArea));
        assert_eq!(d.secondary.as_deref(), Some("Nasjonalpark"));
    }

    #[test]
    fn search_finds_and_ranks_through_forward_search() {
        let bundle = Bundle::open(&build_fixture()).unwrap();
        let hits = bundle.search("galdh", 5).unwrap();

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].title, "Galdhøpiggen");
        assert_eq!(hits[0].icon, "mountain");
        assert!(hits[0].description.as_deref().unwrap().contains("Lom"));
    }
}
