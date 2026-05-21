import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';

import '../models/tide_forecast.dart';
import 'kartverket_tide_service.dart';

final kartverketTideServiceProvider =
    Provider<KartverketTideService>((ref) => KartverketTideService());

/// Async family yielding tide predictions for a coordinate, or `null`
/// when the location is outside Kartverket's coverage. Errors collapse
/// to `null` for the same reason — the ocean tab degrades gracefully.
final tideForecastProvider = AsyncNotifierProvider.family<
    TideForecastNotifier, TideForecast?, LatLng>(
  TideForecastNotifier.new,
);

class TideForecastNotifier extends AsyncNotifier<TideForecast?> {
  TideForecastNotifier(this.position);

  final LatLng position;
  TideForecast? _cached;

  @override
  Future<TideForecast?> build() async {
    final cached = _cached;
    if (cached != null && cached.isFresh) return cached;
    try {
      final result =
          await ref.read(kartverketTideServiceProvider).fetch(position);
      _cached = result;
      return result;
    } catch (_) {
      return null;
    }
  }

  Future<void> refresh() async {
    _cached = null;
    ref.invalidateSelf();
    await future;
  }
}
