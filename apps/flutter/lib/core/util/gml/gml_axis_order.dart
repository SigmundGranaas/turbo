/// Axis order for a given OGC `srsName` value.
///
/// GeoJSON consumes coordinates in **[longitude, latitude]** order
/// (RFC 7946 §3.1.1). GML, by contrast, follows whatever order the CRS
/// dictionary declares — and for EPSG:4326 that's `latitude longitude`.
/// The legacy short form `EPSG:4326` (without "urn") is interpreted by
/// most servers as `longitude latitude` for backwards compatibility.
///
/// This enum captures only what the converter actually needs: should we
/// swap the first two tokens of a `gml:posList` pair before emitting
/// them as GeoJSON `[x, y]`?
enum GmlAxisOrder {
  /// Input is `lat lon` (urn-form EPSG:4326 and friends). Swap to
  /// produce GeoJSON `[lon, lat]`.
  latLon,

  /// Input is already `lon lat` / `easting northing`. Pass through.
  lonLat,
}

extension GmlAxisOrderFor on String {
  /// Decide axis order from an OGC `srsName` string. Empty / unknown
  /// values default to [GmlAxisOrder.lonLat] — that's the GeoJSON
  /// invariant and the safest fallback (any swap is reversible if
  /// downstream detects it, but a missed swap silently puts
  /// coordinates in the wrong hemisphere).
  GmlAxisOrder get gmlAxisOrder {
    final s = toLowerCase();
    // urn-form EPSG references honour the dictionary axis order:
    //   urn:ogc:def:crs:EPSG::4326    → lat lon
    //   urn:x-ogc:def:crs:EPSG:6.6:4326 → lat lon
    if (s.startsWith('urn:ogc:def:crs:epsg') ||
        s.startsWith('urn:x-ogc:def:crs:epsg')) {
      // EPSG:4326 / 4258 / 4269 are all lat,lon under the dictionary.
      // UTM zones (25832/25833/etc.) are easting,northing — that's
      // already "x,y" so we pass through as lonLat too. This branch
      // only swaps for the geographic CRSes the dataset actually
      // uses.
      if (s.endsWith(':4326') || s.endsWith(':4258') || s.endsWith(':4269')) {
        return GmlAxisOrder.latLon;
      }
      return GmlAxisOrder.lonLat;
    }
    // http://www.opengis.net/def/crs/EPSG/0/4326 → lat lon
    if (s.startsWith('http://www.opengis.net/def/crs/epsg')) {
      if (s.endsWith('/4326') || s.endsWith('/4258') || s.endsWith('/4269')) {
        return GmlAxisOrder.latLon;
      }
      return GmlAxisOrder.lonLat;
    }
    // Legacy short forms `EPSG:4326` are widely interpreted as lon,lat.
    return GmlAxisOrder.lonLat;
  }
}
