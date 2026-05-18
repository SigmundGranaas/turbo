import 'package:http/http.dart' as http;
import 'package:latlong2/latlong.dart';
import 'package:xml/xml.dart';

import 'package:turbo/core/util/user_agent.dart';
import '../models/tide_forecast.dart';

/// Thin wrapper around Kartverket's sehavniva tide-prediction endpoint.
///
/// Coverage is Norwegian coastal waters only. Outside that footprint the
/// API returns no `<waterlevel>` rows, which translates to a `null` result
/// here so the caller can silently hide the tide UI without an error
/// state — same shape as [YrOceanService].
class KartverketTideService {
  static const String _host = 'vannstand.kartverket.no';
  static const String _path = '/tideapi.php';
  static const Duration _cacheTtl = Duration(hours: 6);

  final http.Client _client;

  KartverketTideService({http.Client? client})
      : _client = client ?? http.Client();

  Future<TideForecast?> fetch(LatLng position) async {
    final now = DateTime.now().toUtc();
    final from = now.subtract(const Duration(hours: 6));
    final to = now.add(const Duration(days: 3));

    final uri = Uri.https(_host, _path, {
      'lat': position.latitude.toStringAsFixed(4),
      'lon': position.longitude.toStringAsFixed(4),
      'fromtime': _formatApiTime(from),
      'totime': _formatApiTime(to),
      // 'tab' returns only high/low extrema, which is what the ocean
      // tab cares about. 60-min interval would give a full curve.
      'datatype': 'tab',
      'refcode': 'cd',
      'lang': 'en',
      'dst': '1',
      'tide_request': 'locationdata',
    });

    final response = await _client.get(
      uri,
      headers: const {
        'User-Agent': kTurboUserAgent,
        'Accept': 'application/xml,text/xml',
      },
    );

    if (response.statusCode < 200 || response.statusCode >= 300) {
      return null;
    }

    final body = response.body.trim();
    if (body.isEmpty) return null;

    final XmlDocument doc;
    try {
      doc = XmlDocument.parse(body);
    } on XmlException {
      return null;
    }

    final waterLevels = doc.findAllElements('waterlevel').toList();
    if (waterLevels.isEmpty) return null;

    final extrema = <TideExtremum>[];
    for (final node in waterLevels) {
      final timeAttr = node.getAttribute('time');
      final valueAttr = node.getAttribute('value');
      final flagAttr = node.getAttribute('flag');
      if (timeAttr == null || valueAttr == null) continue;
      final time = DateTime.tryParse(timeAttr);
      final value = double.tryParse(valueAttr);
      if (time == null || value == null) continue;
      final kind = _kindFromFlag(flagAttr);
      if (kind == null) continue;
      extrema.add(TideExtremum(
        timeUtc: time.toUtc(),
        levelCm: value,
        kind: kind,
      ));
    }
    if (extrema.isEmpty) return null;
    extrema.sort((a, b) => a.timeUtc.compareTo(b.timeUtc));

    return TideForecast(
      stationName: _readStationName(doc),
      extrema: extrema,
      fetchedAt: now,
      expiresAt: now.add(_cacheTtl),
    );
  }

  static TideKind? _kindFromFlag(String? flag) {
    switch (flag) {
      case 'high':
      case 'High':
      case 'HW':
        return TideKind.high;
      case 'low':
      case 'Low':
      case 'LW':
        return TideKind.low;
    }
    return null;
  }

  static String? _readStationName(XmlDocument doc) {
    final location = doc.findAllElements('location').firstOrNull;
    return location?.getAttribute('name');
  }

  /// Kartverket wants `yyyy-MM-ddTHH:mm` without seconds or zone.
  static String _formatApiTime(DateTime t) {
    final l = t.toLocal();
    String p(int n) => n.toString().padLeft(2, '0');
    return '${l.year}-${p(l.month)}-${p(l.day)}T${p(l.hour)}:${p(l.minute)}';
  }
}
