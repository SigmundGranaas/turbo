/// One predicted high or low tide event.
class TideExtremum {
  final DateTime timeUtc;

  /// Sea-level height for this extremum, in centimetres above chart
  /// datum (Kartverket convention).
  final double levelCm;
  final TideKind kind;

  const TideExtremum({
    required this.timeUtc,
    required this.levelCm,
    required this.kind,
  });
}

enum TideKind { high, low }

/// 3-day tide prediction series returned by Kartverket's sehavniva API.
///
/// Outside Norway the API returns no data — in that case the upstream
/// service yields `null` and this model isn't constructed at all.
class TideForecast {
  final String? stationName;
  final List<TideExtremum> extrema;
  final DateTime fetchedAt;
  final DateTime expiresAt;

  const TideForecast({
    required this.stationName,
    required this.extrema,
    required this.fetchedAt,
    required this.expiresAt,
  });

  bool get isFresh => DateTime.now().toUtc().isBefore(expiresAt);

  /// Returns the extrema falling on the same local day as [day].
  List<TideExtremum> forLocalDay(DateTime day) {
    return [
      for (final e in extrema)
        if (_sameLocalDay(e.timeUtc, day)) e,
    ];
  }

  /// Next extremum strictly after [from]. Used to drive the "next high/low
  /// in 3h 14m" cue on the ocean tab.
  TideExtremum? nextAfter(DateTime from) {
    final fromUtc = from.toUtc();
    for (final e in extrema) {
      if (e.timeUtc.isAfter(fromUtc)) return e;
    }
    return null;
  }
}

bool _sameLocalDay(DateTime utc, DateTime localDay) {
  final l = utc.toLocal();
  return l.year == localDay.year &&
      l.month == localDay.month &&
      l.day == localDay.day;
}
