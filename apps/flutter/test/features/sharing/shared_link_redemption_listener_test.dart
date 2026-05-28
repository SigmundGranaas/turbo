import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/features/sharing/api.dart';

import 'fakes/fake_sharing_api.dart';

/// FakeSharingApi extension scoped to redemption tests — tracks the
/// token presented to /redeem and returns a canned LinkRedemption.
class _RedeemingApi extends FakeSharingApi {
  final List<String> redeemedTokens = [];
  LinkRedemption? response;
  Object? failure;

  @override
  Future<LinkRedemption> redeemLink(String token) async {
    redeemedTokens.add(token);
    if (failure != null) throw failure!;
    return response ??
        const LinkRedemption(
          resourceId: 'r1',
          resourceType: 'collection',
          role: 'editor',
        );
  }
}

ProviderContainer _container({
  required _RedeemingApi api,
  required bool sharingAvailable,
}) {
  return ProviderContainer(overrides: [
    sharingAvailableProvider.overrideWith((_) => sharingAvailable),
    sharingApiClientProvider.overrideWith((_) => api),
  ]);
}

Widget _wrap(ProviderContainer container) {
  return UncontrolledProviderScope(
    container: container,
    child: MaterialApp(
      localizationsDelegates: const [
        AppLocalizations.delegate,
        GlobalMaterialLocalizations.delegate,
        GlobalWidgetsLocalizations.delegate,
        GlobalCupertinoLocalizations.delegate,
      ],
      supportedLocales: AppLocalizations.supportedLocales,
      home: SharedLinkRedemptionListener(
        child: const Scaffold(body: Text('home')),
      ),
    ),
  );
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  group('SharedLinkRedemptionListener — cold-start redemption', () {
    testWidgets(
        'pending token + sharing available → redeems and shows confirmation snackbar',
        (tester) async {
      final api = _RedeemingApi()
        ..response = const LinkRedemption(
          resourceId: 'r1',
          resourceType: 'collection',
          role: 'editor',
        );
      final container = _container(api: api, sharingAvailable: true);
      addTearDown(container.dispose);
      // Stage the token BEFORE mounting the widget tree — this is the
      // cold-start case: ShareRouteHandler runs in main() and pushes the
      // token onto the provider before the first frame.
      container.read(pendingLinkRedemptionProvider.notifier).push('tok-abc');

      await tester.pumpWidget(_wrap(container));
      await tester.pumpAndSettle();

      expect(api.redeemedTokens.single, 'tok-abc');
      expect(
        find.textContaining('A collection was shared with you'),
        findsOneWidget,
      );
    });

    testWidgets(
        'sharing unavailable (anonymous) → does NOT redeem, token stays pending',
        (tester) async {
      final api = _RedeemingApi();
      final container = _container(api: api, sharingAvailable: false);
      addTearDown(container.dispose);
      container.read(pendingLinkRedemptionProvider.notifier).push('tok-anon');

      await tester.pumpWidget(_wrap(container));
      await tester.pumpAndSettle();

      expect(api.redeemedTokens, isEmpty);
      expect(container.read(pendingLinkRedemptionProvider), 'tok-anon');
    });

    testWidgets('server rejects the token → user sees a friendly error',
        (tester) async {
      final api = _RedeemingApi()..failure = Exception('Link invalid');
      final container = _container(api: api, sharingAvailable: true);
      addTearDown(container.dispose);
      container.read(pendingLinkRedemptionProvider.notifier).push('tok-bad');

      await tester.pumpWidget(_wrap(container));
      await tester.pumpAndSettle();

      expect(api.redeemedTokens.single, 'tok-bad');
      expect(
        find.textContaining('share link is no longer valid'),
        findsOneWidget,
      );
    });

    testWidgets('marker payload type → "marker was shared" copy', (tester) async {
      final api = _RedeemingApi()
        ..response = const LinkRedemption(
          resourceId: 'r2',
          resourceType: 'marker',
          role: 'viewer',
        );
      final container = _container(api: api, sharingAvailable: true);
      addTearDown(container.dispose);
      container.read(pendingLinkRedemptionProvider.notifier).push('tok-marker');

      await tester.pumpWidget(_wrap(container));
      await tester.pumpAndSettle();

      expect(
        find.textContaining('A marker was shared with you'),
        findsOneWidget,
      );
    });

    testWidgets('path payload type → "route was shared" copy', (tester) async {
      final api = _RedeemingApi()
        ..response = const LinkRedemption(
          resourceId: 'r3',
          resourceType: 'path',
          role: 'editor',
        );
      final container = _container(api: api, sharingAvailable: true);
      addTearDown(container.dispose);
      container.read(pendingLinkRedemptionProvider.notifier).push('tok-path');

      await tester.pumpWidget(_wrap(container));
      await tester.pumpAndSettle();

      expect(
        find.textContaining('A route was shared with you'),
        findsOneWidget,
      );
    });
  });

  group('SharedLinkRedemptionListener — link arrives while running', () {
    testWidgets('token pushed AFTER mount also gets redeemed', (tester) async {
      final api = _RedeemingApi()
        ..response = const LinkRedemption(
          resourceId: 'r3',
          resourceType: 'path',
          role: 'editor',
        );
      final container = _container(api: api, sharingAvailable: true);
      addTearDown(container.dispose);

      await tester.pumpWidget(_wrap(container));
      await tester.pumpAndSettle();
      expect(api.redeemedTokens, isEmpty);

      // Simulate a deep link arriving while the app is running (e.g. user
      // taps a /share/r/ link from a messenger; ShareRouteHandler decodes
      // it from the app_links stream and pushes the token).
      container
          .read(pendingLinkRedemptionProvider.notifier)
          .push('tok-running');
      await tester.pumpAndSettle();

      expect(api.redeemedTokens.single, 'tok-running');
    });
  });
}
