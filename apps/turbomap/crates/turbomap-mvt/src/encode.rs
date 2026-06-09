//! MVT *encoder* — builds spec-compliant vector-tile bytes from plain
//! geometry. The inverse of [`crate::decode`].
//!
//! Exists so the system can be exercised end-to-end without any external
//! tile server: synthetic worlds (test basemaps, simulator sessions) are
//! encoded to real MVT protobuf and pushed through the same byte-level
//! ingest path a production host uses. Round-tripping through the decoder
//! is the correctness contract (see tests).

use std::collections::HashMap;

use prost::Message as _;

use crate::proto;
use crate::Value;

/// Builds one MVT tile from layers of features.
#[derive(Default)]
pub struct TileEncoder {
    layers: Vec<proto::tile::Layer>,
}

impl TileEncoder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Start a layer. Finish it with [`LayerEncoder::finish`].
    pub fn layer(self, name: &str, extent: u32) -> LayerEncoder {
        LayerEncoder {
            tile: self,
            layer: proto::tile::Layer {
                version: 2,
                name: name.to_string(),
                features: Vec::new(),
                keys: Vec::new(),
                values: Vec::new(),
                extent: Some(extent),
            },
            key_index: HashMap::new(),
            value_index: HashMap::new(),
        }
    }

    /// Encode to protobuf bytes.
    pub fn finish(self) -> Vec<u8> {
        proto::Tile {
            layers: self.layers,
        }
        .encode_to_vec()
    }
}

/// Adds features to one layer. Coordinates are tile-local (`0..extent`).
pub struct LayerEncoder {
    tile: TileEncoder,
    layer: proto::tile::Layer,
    key_index: HashMap<String, u32>,
    value_index: HashMap<ValueKey, u32>,
}

/// Hashable stand-in for `Value` (f64 isn't `Eq`; we key floats by bits).
#[derive(Clone, PartialEq, Eq, Hash)]
enum ValueKey {
    String(String),
    FloatBits(u64),
    Int(i64),
    UInt(u64),
    Bool(bool),
}

impl LayerEncoder {
    pub fn point(self, xy: (i32, i32), props: &[(&str, Value)]) -> Self {
        let geometry = encode_points(&[xy]);
        self.push(proto::tile::GeomType::Point, geometry, props)
    }

    pub fn line(self, vertices: &[(i32, i32)], props: &[(&str, Value)]) -> Self {
        let geometry = encode_path(vertices, false);
        self.push(proto::tile::GeomType::Linestring, geometry, props)
    }

    /// One closed ring (don't repeat the first vertex; ClosePath is added).
    pub fn polygon(self, ring: &[(i32, i32)], props: &[(&str, Value)]) -> Self {
        let geometry = encode_path(ring, true);
        self.push(proto::tile::GeomType::Polygon, geometry, props)
    }

    fn push(
        mut self,
        geom_type: proto::tile::GeomType,
        geometry: Vec<u32>,
        props: &[(&str, Value)],
    ) -> Self {
        let mut tags = Vec::with_capacity(props.len() * 2);
        for (key, value) in props {
            tags.push(self.intern_key(key));
            tags.push(self.intern_value(value));
        }
        self.layer.features.push(proto::tile::Feature {
            id: Some(self.layer.features.len() as u64 + 1),
            tags,
            r#type: Some(geom_type as i32),
            geometry,
        });
        self
    }

    fn intern_key(&mut self, key: &str) -> u32 {
        if let Some(&i) = self.key_index.get(key) {
            return i;
        }
        let i = self.layer.keys.len() as u32;
        self.layer.keys.push(key.to_string());
        self.key_index.insert(key.to_string(), i);
        i
    }

    fn intern_value(&mut self, value: &Value) -> u32 {
        let key = match value {
            Value::String(s) => ValueKey::String(s.clone()),
            Value::Float(f) => ValueKey::FloatBits(f.to_bits()),
            Value::Int(i) => ValueKey::Int(*i),
            Value::UInt(u) => ValueKey::UInt(*u),
            Value::Bool(b) => ValueKey::Bool(*b),
            Value::Null => ValueKey::Int(i64::MIN), // never emitted below
        };
        if let Some(&i) = self.value_index.get(&key) {
            return i;
        }
        let i = self.layer.values.len() as u32;
        self.layer.values.push(encode_value(value));
        self.value_index.insert(key, i);
        i
    }

    /// Close the layer, returning the tile builder.
    pub fn finish(mut self) -> TileEncoder {
        self.tile.layers.push(self.layer);
        self.tile
    }
}

fn encode_value(value: &Value) -> proto::tile::Value {
    let mut out = proto::tile::Value::default();
    match value {
        Value::String(s) => out.string_value = Some(s.clone()),
        Value::Float(f) => out.double_value = Some(*f),
        Value::Int(i) => out.int_value = Some(*i),
        Value::UInt(u) => out.uint_value = Some(*u),
        Value::Bool(b) => out.bool_value = Some(*b),
        Value::Null => {}
    }
    out
}

fn zigzag_encode(n: i32) -> u32 {
    ((n << 1) ^ (n >> 31)) as u32
}

fn command(id: u32, count: u32) -> u32 {
    (count << 3) | id
}

fn encode_points(points: &[(i32, i32)]) -> Vec<u32> {
    let mut out = vec![command(1, points.len() as u32)];
    let mut cursor = (0, 0);
    for &(x, y) in points {
        out.push(zigzag_encode(x - cursor.0));
        out.push(zigzag_encode(y - cursor.1));
        cursor = (x, y);
    }
    out
}

fn encode_path(vertices: &[(i32, i32)], close: bool) -> Vec<u32> {
    assert!(vertices.len() >= 2, "a path needs at least two vertices");
    let mut out = Vec::with_capacity(vertices.len() * 2 + 3);
    // MoveTo the first vertex.
    out.push(command(1, 1));
    out.push(zigzag_encode(vertices[0].0));
    out.push(zigzag_encode(vertices[0].1));
    // LineTo the rest.
    out.push(command(2, vertices.len() as u32 - 1));
    let mut cursor = vertices[0];
    for &(x, y) in &vertices[1..] {
        out.push(zigzag_encode(x - cursor.0));
        out.push(zigzag_encode(y - cursor.1));
        cursor = (x, y);
    }
    if close {
        out.push(command(7, 1));
    }
    out
}

#[cfg(test)]
mod tests {
    //! The encoder's contract is the decoder: everything we encode must
    //! come back identical through `crate::decode`.

    use super::*;
    use crate::{decode, GeomType, Geometry};

    #[test]
    fn point_line_polygon_round_trip_through_the_decoder() {
        let bytes = TileEncoder::new()
            .layer("places", 4096)
            .point((1024, 2048), &[("name", Value::String("Bergen".into()))])
            .finish()
            .layer("roads", 4096)
            .line(
                &[(0, 100), (2000, 150), (4095, 90)],
                &[("kind", Value::String("road".into())), ("lanes", Value::Int(2))],
            )
            .finish()
            .layer("water", 4096)
            .polygon(&[(500, 500), (3500, 500), (3500, 3000), (500, 3000)], &[])
            .finish()
            .finish();

        let tile = decode(&bytes).expect("decode what we encoded");
        assert_eq!(tile.layers.len(), 3);

        let places = &tile.layers[0];
        assert_eq!(places.name, "places");
        assert_eq!(places.features[0].geom_type, GeomType::Point);
        assert_eq!(
            places.features[0].geometry,
            Geometry::Point(vec![(1024, 2048)])
        );
        assert_eq!(
            places.features[0].properties.get("name"),
            Some(&Value::String("Bergen".into()))
        );

        let roads = &tile.layers[1];
        assert_eq!(
            roads.features[0].geometry,
            Geometry::LineString(vec![vec![(0, 100), (2000, 150), (4095, 90)]])
        );
        assert_eq!(roads.features[0].properties.get("lanes"), Some(&Value::Int(2)));

        let water = &tile.layers[2];
        match &water.features[0].geometry {
            Geometry::Polygon(rings) => {
                assert_eq!(rings.len(), 1);
                assert_eq!(rings[0].first(), Some(&(500, 500)));
                assert_eq!(rings[0].last(), Some(&(500, 500)), "ring closed");
            }
            other => panic!("expected polygon, got {other:?}"),
        }
    }

    #[test]
    fn properties_are_interned_not_duplicated() {
        let bytes = TileEncoder::new()
            .layer("roads", 4096)
            .line(&[(0, 0), (10, 10)], &[("kind", Value::String("road".into()))])
            .line(&[(0, 5), (10, 15)], &[("kind", Value::String("road".into()))])
            .finish()
            .finish();
        let tile = decode(&bytes).expect("decode");
        // Both features share the single interned key/value pair.
        assert_eq!(tile.layers[0].features.len(), 2);
        for f in &tile.layers[0].features {
            assert_eq!(f.properties.get("kind"), Some(&Value::String("road".into())));
        }
    }
}
