use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RouteStatus {
    Draft,
    Published,
    Archived,
}

/// Cost profile for routing queries. Cost functions live in
/// `turbo-tiles-routing::cost_fn` and are applied at query time over
/// `paths.edge`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Profile {
    Hiking,
    Ski,
    BikeGravel,
    BikeRoad,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CuratedRouteRef {
    pub id: Uuid,
    pub name: Option<String>,
    pub status: RouteStatus,
}
