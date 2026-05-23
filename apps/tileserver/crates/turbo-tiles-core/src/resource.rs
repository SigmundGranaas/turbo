use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// One of the curated resources surfaced by the API. The slug is the URL
/// segment (`/v1/{slug}/...`) and the storage discriminator in
/// `paths.curated_route.resource`. Keep these in sync with the SQL views
/// in `migrations/20260601_0005_views_resources.sql`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Resource {
    HikingTrails,
    SkiTracks,
    ForestRoads,
    CyclingRoutes,
}

impl Resource {
    pub const ALL: [Resource; 4] = [
        Resource::HikingTrails,
        Resource::SkiTracks,
        Resource::ForestRoads,
        Resource::CyclingRoutes,
    ];

    pub fn slug(self) -> &'static str {
        match self {
            Resource::HikingTrails => "hiking-trails",
            Resource::SkiTracks => "ski-tracks",
            Resource::ForestRoads => "forest-roads",
            Resource::CyclingRoutes => "cycling-routes",
        }
    }

    /// Database view name backing this resource (combined edge subset +
    /// curated rows). Defined in migration 0005.
    pub fn view(self) -> &'static str {
        match self {
            Resource::HikingTrails => "paths.v_hiking_trails",
            Resource::SkiTracks => "paths.v_ski_tracks",
            Resource::ForestRoads => "paths.v_forest_roads",
            Resource::CyclingRoutes => "paths.v_cycling_routes",
        }
    }
}

impl fmt::Display for Resource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.slug())
    }
}

impl FromStr for Resource {
    type Err = UnknownResource;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "hiking-trails" => Ok(Resource::HikingTrails),
            "ski-tracks" => Ok(Resource::SkiTracks),
            "forest-roads" => Ok(Resource::ForestRoads),
            "cycling-routes" => Ok(Resource::CyclingRoutes),
            _ => Err(UnknownResource(s.to_string())),
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("unknown resource `{0}`")]
pub struct UnknownResource(pub String);

/// What `/v1/catalog` returns per resource. Localized labels are
/// supplied for `nb` (Norwegian Bokmål) and `en`; the Flutter client
/// picks based on its locale.
#[derive(Debug, Serialize)]
pub struct ResourceDescriptor {
    pub id: &'static str,
    pub name: NameI18n,
    pub geometry_types: &'static [&'static str],
    pub default_zoom_range: ZoomRange,
    pub attribution: &'static str,
    pub tiles_url_template: String,
    pub geojson_url_template: String,
}

#[derive(Debug, Serialize)]
pub struct NameI18n {
    pub nb: &'static str,
    pub en: &'static str,
}

#[derive(Debug, Serialize)]
pub struct ZoomRange {
    pub min: u8,
    pub max: u8,
}

impl Resource {
    pub fn descriptor(self, base_url: &str) -> ResourceDescriptor {
        let (name, attribution) = match self {
            Resource::HikingTrails => (
                NameI18n {
                    nb: "Turstier",
                    en: "Hiking trails",
                },
                "© Kartverket, Nasjonal Turbase",
            ),
            Resource::SkiTracks => (
                NameI18n {
                    nb: "Skiløyper",
                    en: "Ski tracks",
                },
                "© Kartverket, Skisporet.no",
            ),
            Resource::ForestRoads => (
                NameI18n {
                    nb: "Skogsbilveier",
                    en: "Forest roads",
                },
                "© Kartverket",
            ),
            Resource::CyclingRoutes => (
                NameI18n {
                    nb: "Sykkelruter",
                    en: "Cycling routes",
                },
                "© Kartverket",
            ),
        };

        let slug = self.slug();
        ResourceDescriptor {
            id: slug,
            name,
            geometry_types: &["LineString", "MultiLineString"],
            default_zoom_range: ZoomRange { min: 8, max: 18 },
            attribution,
            tiles_url_template: format!("{base_url}/v1/{slug}/tiles/{{z}}/{{x}}/{{y}}.mvt"),
            geojson_url_template: format!("{base_url}/v1/{slug}/{{id}}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_four_resources_round_trip() {
        // Every entry in Resource::ALL must parse back to itself.
        // Regression guard: a new variant must also extend `slug`
        // and the FromStr arms.
        for r in Resource::ALL {
            let parsed: Resource = r.slug().parse().unwrap();
            assert_eq!(parsed, r);
        }
    }

    #[test]
    fn rejects_unknown_resource() {
        assert!("ferrata".parse::<Resource>().is_err());
        assert!("".parse::<Resource>().is_err());
        // The slugs are kebab-case; underscored / CamelCase variants
        // must be rejected, not silently accepted.
        assert!("hiking_trails".parse::<Resource>().is_err());
        assert!("HikingTrails".parse::<Resource>().is_err());
    }

    #[test]
    fn view_names_are_schema_qualified() {
        // The `paths.` prefix is the schema the MVT query depends
        // on. If anyone moves the views, this test catches it.
        for r in Resource::ALL {
            assert!(r.view().starts_with("paths.v_"));
        }
    }

    #[test]
    fn descriptor_template_contains_z_x_y() {
        // The Flutter client interpolates `{z}/{x}/{y}` literally;
        // missing any placeholder silently breaks tile rendering.
        let d = Resource::HikingTrails.descriptor("http://example.com");
        assert!(d.tiles_url_template.contains("{z}"));
        assert!(d.tiles_url_template.contains("{x}"));
        assert!(d.tiles_url_template.contains("{y}"));
        assert!(d.geojson_url_template.contains("{id}"));
        assert!(d.tiles_url_template.starts_with("http://example.com"));
    }
}
