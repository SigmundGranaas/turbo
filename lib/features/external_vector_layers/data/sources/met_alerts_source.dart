import 'package:flutter/material.dart';

import 'package:turbo/app/l10n/app_localizations.dart';
import '../../models/vector_layer_source.dart';

/// Source descriptor for MET Norway's MetAlerts 2.0 GeoJSON feed.
///
/// MetAlerts doesn't actually accept a bbox parameter — the endpoint is small
/// enough to return the entire active set globally. We bypass the bbox in
/// the URL and let the repository tile / cache it normally; the in-memory
/// cache makes the repeated full responses cheap.
VectorLayerSource metAlertsVectorSource() {
  return VectorLayerSource(
    id: 'metalerts_vector',
    name: (c) => c.l10n.layerNameWeatherAlerts,
    color: const Color(0xFFFF6F00),
    persist: false,
    buildUri: ({
      required minLat,
      required minLon,
      required maxLat,
      required maxLon,
      maxFeatures,
    }) {
      return Uri.https('api.met.no', '/weatherapi/metalerts/2.0/current.json');
    },
  );
}
