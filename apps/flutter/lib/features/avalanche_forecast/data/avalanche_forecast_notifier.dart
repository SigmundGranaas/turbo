import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';

import '../models/avalanche_warning.dart';
import 'varsom_service.dart';

final varsomServiceProvider =
    Provider<VarsomService>((ref) => VarsomService());

/// Family notifier keyed on [LatLng] holding today's Varsom warning, if one
/// is available for the coordinate.
///
/// Coverage is sparse: only Norwegian mountain regions are forecast.
/// Outside coverage the future resolves to `null` and the consuming widget
/// (the avalanche badge) hides itself.
final avalancheForecastProvider = AsyncNotifierProvider.family<
    AvalancheForecastNotifier, AvalancheWarning?, LatLng>(
  AvalancheForecastNotifier.new,
);

class AvalancheForecastNotifier
    extends AsyncNotifier<AvalancheWarning?> {
  AvalancheForecastNotifier(this.position);

  final LatLng position;
  AvalancheWarning? _cached;
  DateTime? _cachedAt;

  /// Varsom updates the warning once per day. A one-hour in-memory window
  /// is comfortably below the daily issuance cadence and lets the UI
  /// re-render without thrashing the API.
  static const Duration _cacheTtl = Duration(hours: 1);

  @override
  Future<AvalancheWarning?> build() async {
    final cachedAt = _cachedAt;
    if (cachedAt != null &&
        DateTime.now().difference(cachedAt) < _cacheTtl) {
      return _cached;
    }
    final warning =
        await ref.read(varsomServiceProvider).forToday(position);
    _cached = warning;
    _cachedAt = DateTime.now();
    return warning;
  }

  Future<void> refresh() async {
    _cached = null;
    _cachedAt = null;
    ref.invalidateSelf();
    await future;
  }
}
