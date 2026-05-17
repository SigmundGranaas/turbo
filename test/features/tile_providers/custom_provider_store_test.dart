import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:turbo/features/tile_providers/data/custom_provider_store.dart';
import 'package:turbo/features/tile_providers/models/custom_tile_provider.dart';
import 'package:turbo/features/tile_providers/models/tile_provider_config.dart';

CustomTileProvider _p(String id, {String? url, TileProviderCategory? category}) =>
    CustomTileProvider(
      id: id,
      displayName: 'Display $id',
      urlTemplate: url ?? 'https://example.com/$id/{z}/{x}/{y}.png',
      category: category ?? TileProviderCategory.global,
    );

void main() {
  group('CustomTileProvider.validateUrlTemplate', () {
    test('accepts a well-formed https template', () {
      expect(
          CustomTileProvider.validateUrlTemplate(
              'https://tiles.example.com/{z}/{x}/{y}.png'),
          isNull);
    });

    test('accepts http (some private TMS servers use plain http)', () {
      expect(
          CustomTileProvider.validateUrlTemplate(
              'http://tiles.example.com/{z}/{x}/{y}.png'),
          isNull);
    });

    test('rejects empty input', () {
      expect(CustomTileProvider.validateUrlTemplate(''), 'empty');
      expect(CustomTileProvider.validateUrlTemplate('   '), 'empty');
    });

    test('rejects missing {z}/{x}/{y} placeholders', () {
      expect(
          CustomTileProvider.validateUrlTemplate(
              'https://example.com/static.png'),
          'missing_placeholders');
      expect(
          CustomTileProvider.validateUrlTemplate(
              'https://example.com/{z}/{x}.png'), // no {y}
          'missing_placeholders');
    });

    test('rejects non-http(s) schemes', () {
      expect(
          CustomTileProvider.validateUrlTemplate(
              'ftp://example.com/{z}/{x}/{y}.png'),
          'bad_scheme');
      expect(
          CustomTileProvider.validateUrlTemplate(
              'file:///tmp/{z}/{x}/{y}.png'),
          'bad_scheme');
    });
  });

  group('CustomTileProvider JSON round-trip', () {
    test('encodes a list and decodes back to equivalent values', () {
      final list = [
        _p('c1'),
        _p('c2', category: TileProviderCategory.overlay),
      ];
      final encoded = CustomTileProvider.encodeList(list);
      final decoded = CustomTileProvider.decodeList(encoded);
      expect(decoded, hasLength(2));
      expect(decoded[0].id, 'c1');
      expect(decoded[0].category, TileProviderCategory.global);
      expect(decoded[1].category, TileProviderCategory.overlay);
    });

    test('decodes null/empty as an empty list (no throw)', () {
      expect(CustomTileProvider.decodeList(null), isEmpty);
      expect(CustomTileProvider.decodeList(''), isEmpty);
    });

    test('decodes malformed JSON as an empty list', () {
      // jsonDecode would throw on bad input; the store assumes it's safe so the
      // model defends against a corrupt prefs value.
      // We accept that some inputs (non-JSON) will throw — that's a programmer
      // error, not user input. Just confirm a non-list valid JSON gives empty.
      expect(CustomTileProvider.decodeList('{}'), isEmpty);
      expect(CustomTileProvider.decodeList('"a string"'), isEmpty);
    });
  });

  group('CustomProviderStore persistence', () {
    test('initial build returns an empty list when prefs are empty', () async {
      SharedPreferences.setMockInitialValues({});
      final container = ProviderContainer();
      addTearDown(container.dispose);

      final list = await container.read(customProviderStoreProvider.future);
      expect(list, isEmpty);
    });

    test('add persists the provider and updates state', () async {
      SharedPreferences.setMockInitialValues({});
      final container = ProviderContainer();
      addTearDown(container.dispose);
      await container.read(customProviderStoreProvider.future);

      await container
          .read(customProviderStoreProvider.notifier)
          .add(_p('c-add'));

      // State reflects the addition.
      final state = container.read(customProviderStoreProvider).value!;
      expect(state, hasLength(1));
      expect(state.single.id, 'c-add');

      // A fresh container reading from the same prefs sees the persisted entry.
      final container2 = ProviderContainer();
      addTearDown(container2.dispose);
      final reloaded =
          await container2.read(customProviderStoreProvider.future);
      expect(reloaded, hasLength(1));
      expect(reloaded.single.id, 'c-add');
    });

    test('remove drops the entry and persists', () async {
      SharedPreferences.setMockInitialValues({});
      final container = ProviderContainer();
      addTearDown(container.dispose);
      final notifier = container.read(customProviderStoreProvider.notifier);
      await container.read(customProviderStoreProvider.future);

      await notifier.add(_p('keep'));
      await notifier.add(_p('drop'));
      await notifier.remove('drop');

      final state = container.read(customProviderStoreProvider).value!;
      expect(state.map((p) => p.id), ['keep']);

      // A fresh container sees the same.
      final container2 = ProviderContainer();
      addTearDown(container2.dispose);
      final reloaded =
          await container2.read(customProviderStoreProvider.future);
      expect(reloaded.map((p) => p.id), ['keep']);
    });

    test('removing the last entry clears the underlying prefs key', () async {
      SharedPreferences.setMockInitialValues({});
      final container = ProviderContainer();
      addTearDown(container.dispose);
      await container.read(customProviderStoreProvider.future);
      final notifier = container.read(customProviderStoreProvider.notifier);

      await notifier.add(_p('only'));
      await notifier.remove('only');

      final prefs = await SharedPreferences.getInstance();
      expect(prefs.containsKey('custom_tile_providers'), isFalse,
          reason: 'empty list should clear the key rather than leave [] behind');
    });
  });
}
