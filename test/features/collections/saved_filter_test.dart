import 'package:flutter_map/flutter_map.dart' show LatLngBounds;
import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/features/collections/api.dart';
import 'package:turbo/features/collections/models/saved_filter.dart';

void main() {
  group('SavedFilter', () {
    test('isEmpty is true when no criteria are set', () {
      expect(const SavedFilter().isEmpty, isTrue);
      expect(const SavedFilter(textQuery: '   ').isEmpty, isTrue);
    });

    test('round-trips through JSON', () {
      final f = SavedFilter(
        textQuery: 'fjell',
        boundingBox: LatLngBounds(
          const LatLng(59.9, 10.6),
          const LatLng(60.1, 10.9),
        ),
        dateFrom: DateTime.utc(2026, 1, 1),
        dateTo: DateTime.utc(2026, 12, 31),
      );
      final restored = SavedFilter.fromJsonString(f.toJsonString());
      expect(restored, isNotNull);
      expect(restored!.textQuery, 'fjell');
      expect(restored.boundingBox?.southWest, const LatLng(59.9, 10.6));
      expect(restored.dateFrom, DateTime.utc(2026, 1, 1));
      expect(restored.dateTo, DateTime.utc(2026, 12, 31));
    });

    test('fromJsonString returns null for malformed input', () {
      expect(SavedFilter.fromJsonString(null), isNull);
      expect(SavedFilter.fromJsonString(''), isNull);
      expect(SavedFilter.fromJsonString('not json'), isNull);
    });
  });

  group('Collection with savedFilter', () {
    test('toLocalMap / fromLocalMap preserves the savedFilter', () {
      final c = Collection(
        uuid: 'c-1',
        name: 'Norway hikes',
        savedFilter: const SavedFilter(textQuery: 'fjell'),
      );
      final round = Collection.fromLocalMap(c.toLocalMap());
      expect(round.isSmart, isTrue);
      expect(round.savedFilter?.textQuery, 'fjell');
    });

    test('collection without savedFilter is not smart', () {
      final c = Collection(uuid: 'c-2', name: 'Plain');
      expect(c.isSmart, isFalse);
      final round = Collection.fromLocalMap(c.toLocalMap());
      expect(round.isSmart, isFalse);
      expect(round.savedFilter, isNull);
    });
  });
}
