import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:turbo/core/widgets/app_dialog.dart';
import 'package:turbo/l10n/app_localizations.dart';

Widget _host(Future<void> Function(BuildContext) onPressed) {
  return MaterialApp(
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
            onPressed: () => onPressed(ctx),
            child: const Text('open'),
          ),
        ),
      ),
    ),
  );
}

void main() {
  group('AppDialog.confirm', () {
    testWidgets('returns true when the confirm action is tapped',
        (tester) async {
      bool? result;
      await tester.pumpWidget(_host((ctx) async {
        result = await AppDialog.confirm(
          ctx,
          title: 'Logout',
          content: 'Are you sure?',
          confirmLabel: 'Logout',
        );
      }));
      await tester.pumpAndSettle();

      await tester.tap(find.text('open'));
      await tester.pumpAndSettle();

      expect(find.text('Logout'), findsWidgets); // title + button
      expect(find.text('Are you sure?'), findsOneWidget);
      expect(find.text('Cancel'), findsOneWidget);

      await tester.tap(find.widgetWithText(FilledButton, 'Logout'));
      await tester.pumpAndSettle();
      expect(result, true);
    });

    testWidgets('returns false when the user taps Cancel', (tester) async {
      bool? result;
      await tester.pumpWidget(_host((ctx) async {
        result = await AppDialog.confirm(
          ctx,
          title: 'Logout',
          content: 'Are you sure?',
        );
      }));
      await tester.pumpAndSettle();

      await tester.tap(find.text('open'));
      await tester.pumpAndSettle();

      await tester.tap(find.widgetWithText(TextButton, 'Cancel'));
      await tester.pumpAndSettle();
      expect(result, false);
    });

    testWidgets('returns false when the dialog is dismissed by barrier-tap',
        (tester) async {
      bool? result;
      await tester.pumpWidget(_host((ctx) async {
        result = await AppDialog.confirm(
          ctx,
          title: 'Logout',
          content: 'Are you sure?',
        );
      }));
      await tester.pumpAndSettle();

      await tester.tap(find.text('open'));
      await tester.pumpAndSettle();
      // Tap outside the dialog to dismiss.
      await tester.tapAt(const Offset(20, 20));
      await tester.pumpAndSettle();
      // showDialog default barrierDismissible is true → returns null →
      // helper coerces to false.
      expect(result, false);
    });
  });

  group('AppDialog.destructive', () {
    testWidgets('shows the destructive label on a red button and returns true '
        'when tapped', (tester) async {
      const error = Color(0xFFAA0000);
      bool? result;
      await tester.pumpWidget(MaterialApp(
        theme: ThemeData.from(
          colorScheme: const ColorScheme.light(error: error),
        ),
        localizationsDelegates: const [
          AppLocalizations.delegate,
          GlobalMaterialLocalizations.delegate,
          GlobalWidgetsLocalizations.delegate,
          GlobalCupertinoLocalizations.delegate,
        ],
        supportedLocales: AppLocalizations.supportedLocales,
        home: Scaffold(
          body: Builder(
            builder: (ctx) => ElevatedButton(
              onPressed: () async {
                result = await AppDialog.destructive(
                  ctx,
                  title: 'Delete region',
                  content: 'This will remove offline data.',
                  destructiveLabel: 'Delete',
                );
              },
              child: const Text('open'),
            ),
          ),
        ),
      ));
      await tester.pumpAndSettle();

      await tester.tap(find.text('open'));
      await tester.pumpAndSettle();

      final FilledButton deleteBtn =
          tester.widget(find.widgetWithText(FilledButton, 'Delete'));
      final bg = deleteBtn.style?.backgroundColor?.resolve(<WidgetState>{});
      expect(bg, error,
          reason: 'destructive button must use the error scheme color');

      await tester.tap(find.widgetWithText(FilledButton, 'Delete'));
      await tester.pumpAndSettle();
      expect(result, true);
    });

    testWidgets('returns false when the user taps Cancel', (tester) async {
      bool? result;
      await tester.pumpWidget(_host((ctx) async {
        result = await AppDialog.destructive(
          ctx,
          title: 'Delete?',
          content: 'Confirm.',
          destructiveLabel: 'Delete',
        );
      }));
      await tester.pumpAndSettle();

      await tester.tap(find.text('open'));
      await tester.pumpAndSettle();
      await tester.tap(find.widgetWithText(TextButton, 'Cancel'));
      await tester.pumpAndSettle();
      expect(result, false);
    });
  });
}
