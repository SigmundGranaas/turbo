import 'dart:math' as math;
import 'dart:typed_data';

import 'package:latlong2/latlong.dart';
import 'package:vector_tile/util/geometry.dart' as vt_geom;
import 'package:vector_tile/vector_tile.dart';

import 'package:turbo/features/external_vector_layers/api.dart';

/// Decodes raw MVT bytes into `VectorFeature`s ready for the existing
/// `VectorDataLayer` widget to render.
///
/// MVT ships feature coordinates in tile-local space (0..extent, default
/// 4096). The decoder unprojects to lon/lat using the standard
/// Web-Mercator slippy-tile arithmetic.
class MvtDecoder {
  /// MVT default extent. The tileserver pins it to 4096 in
  /// `turbo-tiles-mvt::tile::render_tile`.
  static const int defaultExtent = 4096;

  /// Decode one tile's protobuf body for a given (z,x,y).
  List<VectorFeature> decode({
    required Uint8List bytes,
    required int z,
    required int x,
    required int y,
  }) {
    if (bytes.isEmpty) return const [];
    final tile = VectorTile.fromBytes(bytes: bytes);
    final out = <VectorFeature>[];
    for (final layer in tile.layers) {
      final extent = layer.extent;
      for (final feature in layer.features) {
        feature.decodeGeometry();
        final properties = _properties(feature);
        final id = _featureId(feature, properties);
        final rings = _decodeRings(feature, extent: extent, z: z, x: x, y: y);
        if (rings.isEmpty) continue;
        final kind = _kind(feature.type);
        if (kind == null) continue;
        out.add(VectorFeature(
          id: id,
          kind: kind,
          rings: rings,
          properties: properties,
        ));
      }
    }
    return out;
  }

  String _featureId(VectorTileFeature feature, Map<String, Object?> props) {
    final attrId = props['id'];
    if (attrId is String && attrId.isNotEmpty) return attrId;
    return feature.id.toString();
  }

  Map<String, Object?> _properties(VectorTileFeature feature) {
    final raw = feature.decodeProperties();
    return raw.map((k, v) => MapEntry(k, v.dartStringValue ?? v.dartIntValue ?? v.dartDoubleValue ?? v.dartBoolValue));
  }

  VectorGeometryKind? _kind(VectorTileGeomType? type) {
    switch (type) {
      case VectorTileGeomType.LINESTRING:
        return VectorGeometryKind.line;
      case VectorTileGeomType.POLYGON:
        return VectorGeometryKind.polygon;
      case VectorTileGeomType.POINT:
      case VectorTileGeomType.UNKNOWN:
      case null:
        return null;
    }
  }

  List<List<LatLng>> _decodeRings(
    VectorTileFeature feature, {
    required int extent,
    required int z,
    required int x,
    required int y,
  }) {
    final result = <List<LatLng>>[];
    final geom = feature.geometry;
    if (geom == null) return result;

    final tileSize = 1 << z;
    final tileLonWest = _tileToLon(x, tileSize);
    final tileLonEast = _tileToLon(x + 1, tileSize);
    final tileLatNorth = _tileToLat(y, tileSize);
    final tileLatSouth = _tileToLat(y + 1, tileSize);
    final dLon = tileLonEast - tileLonWest;
    final dLat = tileLatSouth - tileLatNorth;

    LatLng toLatLng(double localX, double localY) {
      final lon = tileLonWest + (localX / extent) * dLon;
      final lat = tileLatNorth + (localY / extent) * dLat;
      return LatLng(lat, lon);
    }

    if (geom is vt_geom.GeometryLineString) {
      result.add([
        for (final p in geom.coordinates) toLatLng(p[0], p[1]),
      ]);
    } else if (geom is vt_geom.GeometryMultiLineString) {
      for (final line in geom.coordinates) {
        result.add([
          for (final p in line) toLatLng(p[0], p[1]),
        ]);
      }
    } else if (geom is vt_geom.GeometryPolygon) {
      // First ring is the outer boundary; interiors aren't styled separately.
      if (geom.coordinates.isNotEmpty) {
        result.add([
          for (final p in geom.coordinates.first) toLatLng(p[0], p[1]),
        ]);
      }
    } else if (geom is vt_geom.GeometryMultiPolygon) {
      final coords = geom.coordinates;
      if (coords != null) {
        for (final polygon in coords) {
          if (polygon.isEmpty) continue;
          result.add([
            for (final p in polygon.first) toLatLng(p[0], p[1]),
          ]);
        }
      }
    }
    return result;
  }

  double _tileToLon(int x, int tileSize) => x / tileSize * 360.0 - 180.0;

  double _tileToLat(int y, int tileSize) {
    final n = math.pi * (1.0 - 2.0 * y / tileSize);
    final latRad = math.atan(_sinh(n));
    return latRad * 180.0 / math.pi;
  }

  double _sinh(double x) => (math.exp(x) - math.exp(-x)) / 2.0;
}
