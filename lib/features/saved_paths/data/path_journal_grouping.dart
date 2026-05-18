import 'package:intl/intl.dart';

/// One group bucket in the journal-style paths list. The bucket key controls
/// section ordering (lower [order] renders first); the [headerBuilder] yields
/// the localized label so the data layer doesn't depend on l10n.
class PathJournalGroup {
  /// Stable sort key — lower numbers render first.
  final int order;

  /// Identifier suitable for keying widgets (e.g. 'today', '2026-05').
  final String key;

  /// Builds the visible header text from a localizations bundle. Callers pass
  /// an object that exposes the relevant string getters.
  final String Function(PathJournalLabels labels) headerBuilder;

  const PathJournalGroup({
    required this.order,
    required this.key,
    required this.headerBuilder,
  });
}

/// Minimal labels interface so the grouping helper can be tested without a
/// BuildContext or the full AppLocalizations.
abstract class PathJournalLabels {
  String get today;
  String get yesterday;
  String get thisWeek;
  String get thisMonth;
}

/// Bucket a date relative to [now] for the paths list. Buckets, in order:
///
///   Today · Yesterday · This week · This month · "Month YYYY" per month.
///
/// The named buckets are mutually exclusive (a date in "Yesterday" is never
/// also in "This week", even though it might satisfy the diff < 7 check).
PathJournalGroup groupForDate(DateTime when, {required DateTime now}) {
  final today = DateTime(now.year, now.month, now.day);
  final d = DateTime(when.year, when.month, when.day);
  final dayDiff = today.difference(d).inDays;

  if (dayDiff == 0) {
    return PathJournalGroup(
      order: 0,
      key: 'today',
      headerBuilder: (l) => l.today,
    );
  }
  if (dayDiff == 1) {
    return PathJournalGroup(
      order: 1,
      key: 'yesterday',
      headerBuilder: (l) => l.yesterday,
    );
  }
  if (dayDiff > 1 && dayDiff < 7) {
    return PathJournalGroup(
      order: 2,
      key: 'this_week',
      headerBuilder: (l) => l.thisWeek,
    );
  }
  if (d.year == now.year && d.month == now.month) {
    return PathJournalGroup(
      order: 3,
      key: 'this_month',
      headerBuilder: (l) => l.thisMonth,
    );
  }

  // Older months: each is `monthsSince` whole calendar months behind the
  // current month. Newer months → smaller `order` → render first. The +100
  // offset keeps every month bucket below the named buckets (0–3).
  final monthsSince =
      (now.year - d.year) * 12 + (now.month - d.month);
  final key = '${d.year}-${d.month.toString().padLeft(2, '0')}';
  return PathJournalGroup(
    order: 100 + monthsSince,
    key: key,
    headerBuilder: (_) => DateFormat.yMMMM().format(d),
  );
}
