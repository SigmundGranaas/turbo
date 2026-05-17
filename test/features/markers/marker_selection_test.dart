import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:turbo/features/markers/api.dart';

void main() {
  group('MarkerSelectionNotifier', () {
    late ProviderContainer container;
    late MarkerSelectionNotifier notifier;

    setUp(() {
      container = ProviderContainer();
      notifier = container.read(markerSelectionProvider.notifier);
    });

    tearDown(() => container.dispose());

    test('starts empty and reports !isActive', () {
      expect(container.read(markerSelectionProvider), isEmpty);
      expect(notifier.isActive, isFalse);
      expect(notifier.count, 0);
    });

    test('toggle adds when absent and removes when present', () {
      notifier.toggle('a');
      expect(container.read(markerSelectionProvider), {'a'});
      expect(notifier.isActive, isTrue);
      expect(notifier.count, 1);

      notifier.toggle('b');
      expect(container.read(markerSelectionProvider), {'a', 'b'});

      notifier.toggle('a');
      expect(container.read(markerSelectionProvider), {'b'});
    });

    test('add is idempotent', () {
      notifier.add('x');
      notifier.add('x');
      expect(container.read(markerSelectionProvider), {'x'});
    });

    test('remove on a missing uuid is a no-op', () {
      notifier.add('x');
      notifier.remove('y');
      expect(container.read(markerSelectionProvider), {'x'});
    });

    test('clear empties the selection', () {
      notifier
        ..add('a')
        ..add('b')
        ..add('c');
      expect(notifier.count, 3);
      notifier.clear();
      expect(container.read(markerSelectionProvider), isEmpty);
      expect(notifier.isActive, isFalse);
    });

    test('contains returns the membership truthfully', () {
      notifier.add('m1');
      expect(notifier.contains('m1'), isTrue);
      expect(notifier.contains('m2'), isFalse);
    });

    test('clearing an already-empty selection does not notify a new state',
        () {
      var notifyCount = 0;
      container.listen(markerSelectionProvider, (_, __) => notifyCount++);

      notifier.clear();
      // Listener fires once for the initial subscription cycle on some
      // versions; we only care that clearing the empty set does NOT push a
      // new identical empty set. Add an item then clear and observe two
      // distinct emissions (add + clear) vs three.
      notifier.add('a'); // emission #1
      notifier.clear(); // emission #2 (clear from non-empty)
      notifier.clear(); // SHOULD be no-op (clear from empty)
      expect(notifyCount, 2);
    });
  });
}
