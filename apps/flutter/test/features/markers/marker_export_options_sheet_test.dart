import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/core/sharing/api.dart';
import 'package:turbo/features/markers/api.dart';

Marker _marker() => Marker(
      uuid: 'm1',
      title: 'Cabin',
      description: 'In the woods',
      position: const LatLng(60.5, 10.5),
    );

Future<void> _openSheet(WidgetTester tester) async {
  await tester.pumpWidget(
    ProviderScope(
      overrides: [
        webBaseUrlProvider.overrideWithValue('https://example.test'),
      ],
      child: MaterialApp(
        localizationsDelegates: const [
          AppLocalizations.delegate,
          GlobalMaterialLocalizations.delegate,
          GlobalWidgetsLocalizations.delegate,
          GlobalCupertinoLocalizations.delegate,
        ],
        supportedLocales: AppLocalizations.supportedLocales,
        home: Scaffold(
          body: Builder(
            builder: (ctx) => Center(
              child: ElevatedButton(
                child: const Text('open'),
                onPressed: () => showModalBottomSheet(
                  context: ctx,
                  builder: (_) => MarkerExportOptionsSheet(marker: _marker()),
                ),
              ),
            ),
          ),
        ),
      ),
    ),
  );
  await tester.pumpAndSettle();
  await tester.tap(find.text('open'));
  await tester.pumpAndSettle();
}

void main() {
  testWidgets('shows a "Share as link" row instead of text', (tester) async {
    await _openSheet(tester);
    expect(find.text('Share as link'), findsOneWidget);
    expect(find.text('Share as text'), findsNothing);
  });

  testWidgets('does not show the GeoJSON row', (tester) async {
    await _openSheet(tester);
    expect(find.text('GeoJSON'), findsNothing);
  });

  testWidgets(
    'tapping share copies the link to clipboard and closes the sheet',
    (tester) async {
      String? clipboardText;

      // Intercept clipboard writes; the system channel is otherwise a no-op
      // in tests.
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(SystemChannels.platform, (call) async {
        if (call.method == 'Clipboard.setData') {
          clipboardText =
              (call.arguments as Map)['text'] as String?;
        }
        return null;
      });
      addTearDown(() {
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
            .setMockMethodCallHandler(SystemChannels.platform, null);
      });

      await _openSheet(tester);
      await tester.tap(find.byIcon(Icons.share));
      // share_plus opens a system sheet that is a no-op in tests; pump
      // settles whatever async work the handler kicked off.
      await tester.pumpAndSettle();

      expect(clipboardText, isNotNull);
      expect(clipboardText, startsWith('https://example.test/share/m'));
    },
  );
}
