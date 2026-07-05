//! Marker store — the pins the host drops on the map.
//!
//! Pulls the `markers: Vec<Marker>` + `next_marker_id: u64` pair off the
//! `Map` god-object into one type that owns the collection, the id-assignment
//! rule (0 means "auto-assign the next id"), and the screen-space hit test.
//! The public [`Marker`]/[`MarkerId`]/[`HitMarker`] types still live on the
//! `Map` facade (they're the crate's API surface) — this just owns the *state*.

use crate::map::{HitMarker, Marker, MarkerId};

/// Owns the map's markers and their id allocation.
///
/// Markers come in two flavours (plan P6.5):
/// - **Ungrouped** — host-imperative pins (`Map::add_marker`). Drawn in the
///   fixed screen-space track on top of the whole frame, like always.
/// - **Grouped** — the instances of a scene-declared circle *layer*
///   (`Map::add_marker_to_layer`); `group` is the owning layer id. Drawn at
///   that layer's stack slot, so the IR's order is the composited order.
///
/// Both flavours share one store because hit-testing is one question — the
/// grouping only decides *where in the frame* a marker paints.
#[derive(Debug, Default)]
pub struct MarkerManager {
    markers: Vec<Marker>,
    /// Owning circle-layer id per grouped marker, keyed by marker id.
    /// Absent = ungrouped (host marker).
    groups: std::collections::HashMap<MarkerId, String>,
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

    /// Insert a marker owned by the circle layer `group` (its stack slot
    /// draws it). Same id rules as [`Self::add`].
    pub fn add_to_group(&mut self, group: &str, marker: Marker) -> MarkerId {
        let id = self.add(marker);
        self.groups.insert(id, group.to_string());
        id
    }

    pub fn remove(&mut self, id: MarkerId) {
        self.markers.retain(|m| m.id != id);
        self.groups.remove(&id);
    }

    /// Remove every marker owned by the circle layer `group`.
    pub fn remove_group(&mut self, group: &str) {
        let groups = &self.groups;
        self.markers
            .retain(|m| groups.get(&m.id).map(String::as_str) != Some(group));
        self.groups.retain(|_, g| g != group);
    }

    pub fn clear(&mut self) {
        self.markers.clear();
        self.groups.clear();
    }

    pub fn all(&self) -> &[Marker] {
        &self.markers
    }

    /// The markers owned by circle layer `group`, in insertion order.
    pub fn in_group<'a>(&'a self, group: &'a str) -> impl Iterator<Item = &'a Marker> + 'a {
        self.markers
            .iter()
            .filter(move |m| self.groups.get(&m.id).map(String::as_str) == Some(group))
    }

    /// Host markers (no owning layer), in insertion order.
    pub fn ungrouped(&self) -> impl Iterator<Item = &Marker> + '_ {
        self.markers
            .iter()
            .filter(move |m| !self.groups.contains_key(&m.id))
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
    fn groups_partition_the_store_and_remove_together() {
        let mut m = MarkerManager::default();
        m.add(marker(0)); // ungrouped host pin (id 1)
        let a = m.add_to_group("pins", marker(0)); // id 2
        m.add_to_group("dots", marker(0)); // id 3
        m.add_to_group("pins", marker(0)); // id 4

        assert_eq!(m.len(), 4, "grouped + ungrouped share one store");
        assert_eq!(m.in_group("pins").count(), 2);
        assert_eq!(m.in_group("dots").count(), 1);
        assert_eq!(m.ungrouped().count(), 1);
        // Insertion order within a group is preserved (draw order).
        let ids: Vec<u64> = m.in_group("pins").map(|mk| mk.id.0).collect();
        assert_eq!(ids, vec![a.0, 4]);

        // Removing a single grouped marker forgets its group entry.
        m.remove(a);
        assert_eq!(m.in_group("pins").count(), 1);

        // Removing the whole group leaves the others untouched.
        m.remove_group("pins");
        assert_eq!(m.len(), 2);
        assert_eq!(m.in_group("pins").count(), 0);
        assert_eq!(m.in_group("dots").count(), 1);
        assert_eq!(m.ungrouped().count(), 1);

        m.clear();
        assert_eq!(m.ungrouped().count(), 0);
        assert_eq!(m.in_group("dots").count(), 0);
    }

    #[test]
    fn grouped_markers_hit_like_any_marker() {
        // The store is the ONE hit-test source: grouping changes where a
        // marker draws, never whether a tap finds it.
        let mut m = MarkerManager::default();
        m.add_to_group("pins", marker(0));
        let hits = m.hit((100.0, 100.0), 0.0, |_| (100.0, 100.0));
        assert_eq!(hits.len(), 1);
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
