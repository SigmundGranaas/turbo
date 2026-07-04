//! Marker store — the pins the host drops on the map.
//!
//! Pulls the `markers: Vec<Marker>` + `next_marker_id: u64` pair off the
//! `Map` god-object into one type that owns the collection, the id-assignment
//! rule (0 means "auto-assign the next id"), and the screen-space hit test.
//! The public [`Marker`]/[`MarkerId`]/[`HitMarker`] types still live on the
//! `Map` facade (they're the crate's API surface) — this just owns the *state*.

use crate::map::{HitMarker, Marker, MarkerId};

/// Owns the map's markers and their id allocation.
#[derive(Debug, Default)]
pub struct MarkerManager {
    markers: Vec<Marker>,
    /// Monotonic high-water mark for auto-assigned ids. A marker added with
    /// `MarkerId(0)` gets `next_id + 1`; a marker added with an explicit id
    /// bumps this so a later auto-assign can't collide with it.
    next_id: u64,
}

impl MarkerManager {
    /// Insert or replace a marker. `MarkerId(0)` means "assign the next id";
    /// any other id is honoured (and replaces an existing marker with that id).
    /// Returns the resolved id.
    pub fn add(&mut self, mut marker: Marker) -> MarkerId {
        if marker.id == MarkerId(0) {
            self.next_id += 1;
            marker.id = MarkerId(self.next_id);
        } else {
            self.next_id = self.next_id.max(marker.id.0);
        }
        let id = marker.id;
        if let Some(slot) = self.markers.iter_mut().find(|m| m.id == id) {
            *slot = marker;
        } else {
            self.markers.push(marker);
        }
        id
    }

    pub fn remove(&mut self, id: MarkerId) {
        self.markers.retain(|m| m.id != id);
    }

    pub fn clear(&mut self) {
        self.markers.clear();
    }

    pub fn all(&self) -> &[Marker] {
        &self.markers
    }

    pub fn is_empty(&self) -> bool {
        self.markers.is_empty()
    }

    pub fn len(&self) -> usize {
        self.markers.len()
    }

    /// Markers under `screen_px` within `tolerance_px`, top-most first
    /// (newest markers sit above older ones). `project` maps a marker's
    /// geographic position to screen pixels — the caller supplies it because
    /// projection needs the live camera + terrain, which live on `Map`.
    pub fn hit(
        &self,
        screen_px: (f64, f64),
        tolerance_px: f64,
        project: impl Fn(crate::geo::LatLng) -> (f64, f64),
    ) -> Vec<HitMarker> {
        let mut out = Vec::new();
        for marker in self.markers.iter().rev() {
            let mp = project(marker.lng_lat);
            let dx = mp.0 - screen_px.0;
            let dy = mp.1 - screen_px.1;
            let r = (marker.radius_px as f64 + tolerance_px).max(0.0);
            if dx * dx + dy * dy <= r * r {
                out.push(HitMarker {
                    id: marker.id,
                    lng_lat: marker.lng_lat,
                    data: marker.data.clone(),
                });
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geo::LatLng;
    use crate::style::Color;

    fn marker(id: u64) -> Marker {
        Marker {
            id: MarkerId(id),
            lng_lat: LatLng::new(67.25, 15.3),
            radius_px: 10.0,
            color: Color::rgb(255, 255, 255),
            data: Default::default(),
        }
    }

    #[test]
    fn id_zero_auto_assigns_monotonically() {
        let mut m = MarkerManager::default();
        assert_eq!(m.add(marker(0)), MarkerId(1));
        assert_eq!(m.add(marker(0)), MarkerId(2));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn explicit_id_replaces_and_bumps_high_water_mark() {
        let mut m = MarkerManager::default();
        m.add(marker(5));
        m.add(marker(5)); // replace, not duplicate
        assert_eq!(m.len(), 1);
        // next auto id must clear the explicit 5
        assert_eq!(m.add(marker(0)), MarkerId(6));
    }

    #[test]
    fn remove_and_clear() {
        let mut m = MarkerManager::default();
        let a = m.add(marker(0));
        m.add(marker(0));
        m.remove(a);
        assert_eq!(m.len(), 1);
        m.clear();
        assert!(m.is_empty());
    }

    #[test]
    fn hit_returns_topmost_first_within_tolerance() {
        let mut m = MarkerManager::default();
        m.add(marker(0)); // id 1
        m.add(marker(0)); // id 2 (newest → top)
                          // Both project to the same point; both within radius.
        let hits = m.hit((100.0, 100.0), 0.0, |_| (100.0, 100.0));
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].id, MarkerId(2));
        // A click far away hits nothing.
        let none = m.hit((9999.0, 9999.0), 0.0, |_| (100.0, 100.0));
        assert!(none.is_empty());
    }
}
