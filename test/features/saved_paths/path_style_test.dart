import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:turbo/features/saved_paths/models/path_style.dart';

void main() {
  group('PathLineStyle', () {
    test('fromKey parses all valid keys', () {
      expect(PathLineStyle.fromKey('solid'), PathLineStyle.solid);
      expect(PathLineStyle.fromKey('dotted'), PathLineStyle.dotted);
      expect(PathLineStyle.fromKey('dashed'), PathLineStyle.dashed);
      expect(PathLineStyle.fromKey('dash_dot'), PathLineStyle.dashDot);
    });

    test('fromKey returns solid for null or unknown', () {
      expect(PathLineStyle.fromKey(null), PathLineStyle.solid);
      expect(PathLineStyle.fromKey('unknown'), PathLineStyle.solid);
      expect(PathLineStyle.fromKey(''), PathLineStyle.solid);
    });

    test('key round-trips through fromKey', () {
      for (final style in PathLineStyle.values) {
        expect(PathLineStyle.fromKey(style.key), style);
      }
    });

    test('toStrokePattern returns correct types', () {
      expect(PathLineStyle.solid.toStrokePattern(), isA<StrokePattern>());
      expect(PathLineStyle.dotted.toStrokePattern(), isA<StrokePattern>());
      expect(PathLineStyle.dashed.toStrokePattern(), isA<StrokePattern>());
      expect(PathLineStyle.dashDot.toStrokePattern(), isA<StrokePattern>());
    });
  });

  group('colorToHex / hexToColor', () {
    test('round-trips for palette colors', () {
      for (final color in pathColorPalette) {
        final hex = colorToHex(color);
        final parsed = hexToColor(hex);
        expect(parsed, isNotNull);
        // Compare RGB channels
        expect((parsed!.r * 255).round(), (color.r * 255).round());
        expect((parsed.g * 255).round(), (color.g * 255).round());
        expect((parsed.b * 255).round(), (color.b * 255).round());
      }
    });

    test('colorToHex produces 6-char uppercase hex', () {
      final hex = colorToHex(const Color(0xFF1976D2));
      expect(hex, '1976D2');
      expect(hex.length, 6);
    });

    test('hexToColor returns null for invalid input', () {
      expect(hexToColor(null), isNull);
      expect(hexToColor(''), isNull);
      expect(hexToColor('abc'), isNull);
      expect(hexToColor('ZZZZZZ'), isNull);
      expect(hexToColor('1234567'), isNull);
    });

    test('hexToColor parses valid 6-char hex', () {
      final color = hexToColor('FF0000');
      expect(color, isNotNull);
      expect((color!.r * 255).round(), 255);
      expect((color.g * 255).round(), 0);
      expect((color.b * 255).round(), 0);
    });
  });
}
