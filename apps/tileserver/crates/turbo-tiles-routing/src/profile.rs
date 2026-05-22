use turbo_tiles_core::route::Profile;

/// Cost function inputs available to all profiles. Materialised columns
/// on `paths.edge` keep this fast — the routing layer doesn't compute
/// derived attributes at query time.
pub struct EdgeCostInputs {
    pub length_m: f64,
    pub elevation_gain_m: f64,
    pub fkb_type: String,
    pub marking: Option<String>,
    pub surface: Option<String>,
}

/// Returns a Postgres SQL expression that evaluates per row to the
/// edge cost under the given profile. Used by `turbo-tiles-routing` to
/// inject as the cost column in `pgr_dijkstra` calls.
pub fn cost_expression(profile: Profile) -> &'static str {
    match profile {
        // Toblerov-ish hiking cost: distance + extra 10× per metre up.
        Profile::Hiking => "length_m + COALESCE(elevation_gain_m, 0) * 10.0",
        // Ski: prefer prepared (skiloype) edges by heavily down-weighting them.
        Profile::Ski => {
            "length_m * CASE WHEN fkb_type = 'skiloype' THEN 0.5 ELSE 2.0 END \
             + COALESCE(elevation_gain_m, 0) * 6.0"
        }
        // Gravel bike: prefer skogsbilveg/traktorveg surface.
        Profile::BikeGravel => {
            "length_m * CASE WHEN fkb_type IN ('skogsbilveg','traktorveg') \
             THEN 1.0 ELSE 3.0 END + COALESCE(elevation_gain_m, 0) * 4.0"
        }
        // Road bike: avoid unpaved.
        Profile::BikeRoad => {
            "length_m * CASE WHEN fkb_type = 'sykkelvei' THEN 1.0 \
             WHEN fkb_type IN ('skogsbilveg','traktorveg','sti') THEN 50.0 \
             ELSE 1.5 END + COALESCE(elevation_gain_m, 0) * 4.0"
        }
    }
}
