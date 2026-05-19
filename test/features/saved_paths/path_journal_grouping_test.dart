import 'package:flutter_test/flutter_test.dart';
import 'package:turbo/features/saved_paths/data/path_journal_grouping.dart';

class _StubLabels implements PathJournalLabels {
  @override
  String get today => 'Today';
  @override
  String get yesterday => 'Yesterday';
  @override
  String get thisWeek => 'This week';
  @override
  String get thisMonth => 'This month';
}

void main() {
  group('groupForDate', () {
    final labels = _StubLabels();
    // Fixed "now" so the test is deterministic.
    final now = DateTime(2026, 5, 18, 12);

    test('same day buckets as "today"', () {
      final g = groupForDate(DateTime(2026, 5, 18, 7), now: now);
      expect(g.key, 'today');
      expect(g.headerBuilder(labels), 'Today');
      expect(g.order, 0);
    });

    test('one day earlier buckets as "yesterday" even if within last 7 days',
        () {
      final g = groupForDate(DateTime(2026, 5, 17, 23), now: now);
      expect(g.key, 'yesterday');
      expect(g.headerBuilder(labels), 'Yesterday');
      expect(g.order, 1);
    });

    test('2–6 days ago buckets as "this week"', () {
      final two = groupForDate(DateTime(2026, 5, 16), now: now);
      final six = groupForDate(DateTime(2026, 5, 12), now: now);
      expect(two.key, 'this_week');
      expect(six.key, 'this_week');
      expect(two.headerBuilder(labels), 'This week');
      expect(two.order, 2);
    });

    test('7+ days ago in same month buckets as "this month"', () {
      final g = groupForDate(DateTime(2026, 5, 1), now: now);
      expect(g.key, 'this_month');
      expect(g.headerBuilder(labels), 'This month');
      expect(g.order, 3);
    });

    test('previous months bucket as "MMMM yyyy" with descending order', () {
      final april = groupForDate(DateTime(2026, 4, 15), now: now);
      final march = groupForDate(DateTime(2026, 3, 15), now: now);

      expect(april.key, '2026-04');
      expect(march.key, '2026-03');
      // Header is a localized "Month yyyy" string.
      expect(april.headerBuilder(labels), contains('April'));
      expect(april.headerBuilder(labels), contains('2026'));
      // April should sort BEFORE March (newer month first → lower order).
      expect(april.order, lessThan(march.order));
      // Both fall after the named buckets.
      expect(april.order, greaterThanOrEqualTo(4));
    });

    test('different years sort correctly (Dec 2025 > Jan 2026 in chrono order)',
        () {
      final dec2025 = groupForDate(DateTime(2025, 12, 31), now: now);
      final jan2026 = groupForDate(DateTime(2026, 1, 5), now: now);
      // January 2026 is more recent → smaller order → renders first.
      expect(jan2026.order, lessThan(dec2025.order));
    });
  });
}
