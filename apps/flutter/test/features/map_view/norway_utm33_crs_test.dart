import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/map_view/data/norway_utm33_crs.dart';

/// Computes the WMTS (TileCol, TileRow) a [crs] resolves for [latLng] at
/// integer [zoom], the same way flutter_map's TileLayer does (pixel / 256).
({int col, int row}) _tile(Crs crs, LatLng latLng, int zoom) {
  final offset = crs.latLngToOffset(latLng, zoom.toDouble());
  return (col: (offset.dx / 256).floor(), row: (offset.dy / 256).floor());
}

void main() {
  group('norwayUtm33Crs grid', () {
    test('has 19 levels (0-18) that are exact halvings', () {
      expect(norwayUtm33Resolutions.length, 19);
      for (var z = 1; z < norwayUtm33Resolutions.length; z++) {
        expect(
          norwayUtm33Resolutions[z - 1] / norwayUtm33Resolutions[z],
          closeTo(2.0, 1e-9),
        );
      }
      // z18 (deepest native) is ~0.0826 m/px — far finer than Web Mercator
      // topo's z18 (~0.6 m/px at the equator).
      expect(norwayUtm33Resolutions.last, closeTo(0.0826416, 1e-6));
    });

    // These TileCol/TileRow values were verified against live Kartverket
    // tiles (each returned a populated z18 PNG), so they pin the origin,
    // resolution and Y-axis orientation of the CRS.
    test('resolves the verified z18 tile for Oslo', () {
      expect(
        _tile(norwayUtm33Crs, const LatLng(59.9139, 10.7522), 18),
        (col: 130578, row: 113278),
      );
    });

    test('resolves the verified z18 tile for Trondheim', () {
      expect(
        _tile(norwayUtm33Crs, const LatLng(63.4305, 10.3951), 18),
        (col: 130946, row: 94731),
      );
    });

    test('resolves the verified z18 tile for Bergen', () {
      expect(
        _tile(norwayUtm33Crs, const LatLng(60.39, 5.32), 18),
        (col: 116643, row: 109277),
      );
    });
  });

  group('crsForProjection', () {
    test('maps projections to the right CRS', () {
      expect(crsForProjection(MapProjection.webMercator), isA<Epsg3857>());
      expect(crsForProjection(MapProjection.utm33).code, 'EPSG:25833');
    });
  });

  group('convertZoomBetweenProjections', () {
    const lat = 62.0; // central Norway

    test('is a no-op when the projection is unchanged', () {
      expect(
        convertZoomBetweenProjections(
          zoom: 14,
          latitude: lat,
          from: MapProjection.utm33,
          to: MapProjection.utm33,
        ),
        14,
      );
    });

    test('UTM33 needs a lower zoom number than Web Mercator for one scale', () {
      // At ~62°N Web Mercator z18 looks like roughly UTM33 z16.
      final utm = convertZoomBetweenProjections(
        zoom: 18,
        latitude: lat,
        from: MapProjection.webMercator,
        to: MapProjection.utm33,
      );
      expect(utm, lessThan(18));
      expect(utm, closeTo(16.2, 0.4));
    });

    test('round-trips back to the original zoom', () {
      final utm = convertZoomBetweenProjections(
        zoom: 15,
        latitude: lat,
        from: MapProjection.webMercator,
        to: MapProjection.utm33,
      );
      final back = convertZoomBetweenProjections(
        zoom: utm,
        latitude: lat,
        from: MapProjection.utm33,
        to: MapProjection.webMercator,
      );
      expect(back, closeTo(15, 1e-9));
    });
  });
}
