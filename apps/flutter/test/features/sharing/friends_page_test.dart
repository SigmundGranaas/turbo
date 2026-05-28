import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:turbo/features/sharing/api.dart';

import 'fakes/fake_sharing_api.dart';

UserProfile _profile(String code) => UserProfile(
      userId: 'me',
      friendCode: code,
      createdAt: DateTime.utc(2026, 1, 1),
    );

Friendship _pending(String otherUserId, {required String initiatorId}) =>
    Friendship(
      otherUserId: otherUserId,
      initiatorId: initiatorId,
      status: FriendshipStatus.pending,
      createdAt: DateTime.utc(2026, 1, 1),
      acceptedAt: null,
    );

Friendship _accepted(String otherUserId) => Friendship(
      otherUserId: otherUserId,
      initiatorId: 'me',
      status: FriendshipStatus.accepted,
      createdAt: DateTime.utc(2026, 1, 1),
      acceptedAt: DateTime.utc(2026, 1, 2),
    );

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  group('FriendsPage — Friends tab', () {
    testWidgets('empty state surfaces a guiding message, not a blank screen',
        (tester) async {
      await tester.pumpWidget(wrapWithSharingFake(const FriendsPage(), FakeSharingApi()));
      await tester.pumpAndSettle();
      expect(find.text('No friends yet.'), findsOneWidget);
    });

    testWidgets('accepted friends show with their "Since" timestamp',
        (tester) async {
      final api = FakeSharingApi()..acceptedFriends = [_accepted('bob')];
      await tester.pumpWidget(wrapWithSharingFake(const FriendsPage(), api));
      await tester.pumpAndSettle();

      expect(find.text('bob'), findsOneWidget);
      expect(find.textContaining('Since'), findsOneWidget,
          reason: 'Showing when the friendship started anchors the user — '
              'an "old friend" feels different from "added yesterday".');
    });

    testWidgets('Remove friend takes them out of the visible list immediately',
        (tester) async {
      final api = FakeSharingApi()..acceptedFriends = [_accepted('bob')];
      await tester.pumpWidget(wrapWithSharingFake(const FriendsPage(), api));
      await tester.pumpAndSettle();

      // Clear acceptedFriends so the refresh after remove shows the empty state.
      api.acceptedFriends = const [];
      await tester.tap(find.byTooltip('Remove'));
      await tester.pumpAndSettle();

      expect(find.text('bob'), findsNothing);
      expect(find.text('No friends yet.'), findsOneWidget);
      expect(api.friendshipRemoves.single, 'bob');
    });
  });

  group('FriendsPage — Pending tab', () {
    testWidgets('empty state when no pending requests', (tester) async {
      await tester.pumpWidget(wrapWithSharingFake(const FriendsPage(), FakeSharingApi()));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Pending'));
      await tester.pumpAndSettle();

      expect(find.text('No pending requests.'), findsOneWidget);
    });

    testWidgets(
        'accepting a request moves it: gone from Pending, present in Friends',
        (tester) async {
      final api = FakeSharingApi()
        ..allFriendships = [_pending('alice', initiatorId: 'alice')];

      await tester.pumpWidget(wrapWithSharingFake(const FriendsPage(), api));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Pending'));
      await tester.pumpAndSettle();
      expect(find.text('alice'), findsOneWidget);

      // Server-side: after accepting, the friendship becomes accepted in
      // both lists. The fake reflects that.
      api.allFriendships = [_accepted('alice')];
      api.acceptedFriends = [_accepted('alice')];

      await tester.tap(find.byTooltip('Accept'));
      await tester.pumpAndSettle();

      // Pending list is now empty.
      expect(find.text('No pending requests.'), findsOneWidget);
      // Switching to Friends tab shows alice.
      await tester.tap(find.widgetWithText(Tab, 'Friends'));
      await tester.pumpAndSettle();
      expect(find.text('alice'), findsOneWidget);

      expect(api.friendshipAccepts.single, 'alice');
    });

    testWidgets('declining (Remove) takes the request out of Pending',
        (tester) async {
      final api = FakeSharingApi()
        ..allFriendships = [_pending('alice', initiatorId: 'alice')];

      await tester.pumpWidget(wrapWithSharingFake(const FriendsPage(), api));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Pending'));
      await tester.pumpAndSettle();

      api.allFriendships = const [];
      await tester.tap(find.byTooltip('Decline'));
      await tester.pumpAndSettle();

      expect(find.text('alice'), findsNothing);
      expect(find.text('No pending requests.'), findsOneWidget);
      expect(api.friendshipRemoves.single, 'alice');
    });
  });

  group('FriendsPage — Add tab', () {
    Future<void> _goToAddTab(WidgetTester tester, FakeSharingApi api) async {
      await tester.pumpWidget(wrapWithSharingFake(const FriendsPage(), api));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Add'));
      await tester.pumpAndSettle();
    }

    testWidgets('shows the user\'s own friend code prefixed with "turbo-"',
        (tester) async {
      final api = FakeSharingApi()..myProfile = _profile('ablekite');
      await _goToAddTab(tester, api);

      expect(find.text('turbo-ablekite'), findsAtLeastNWidgets(1));
    });

    testWidgets(
        'Copy button copies turbo-<code> and shows "Code copied" confirmation',
        (tester) async {
      final clipboardTexts = <String>[];
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(SystemChannels.platform, (call) async {
        if (call.method == 'Clipboard.setData') {
          clipboardTexts.add((call.arguments as Map)['text'] as String);
        }
        return null;
      });

      final api = FakeSharingApi()..myProfile = _profile('ablekite');
      await _goToAddTab(tester, api);

      await tester.tap(find.byTooltip('Copy'));
      await tester.pumpAndSettle();

      expect(clipboardTexts.single, 'turbo-ablekite',
          reason: 'Pasting bare code without prefix loses the namespace cue; '
              'the user expects to paste exactly what they see.');
      expect(find.text('Code copied'), findsOneWidget,
          reason: 'Copy actions need a confirmation, otherwise users tap twice.');
    });

    testWidgets(
        'friend request flow: lookup, request, success snackbar, field cleared',
        (tester) async {
      final api = FakeSharingApi()..myProfile = _profile('ablekite');
      await _goToAddTab(tester, api);

      await tester.enterText(
          find.widgetWithText(TextField, 'Friend code'), 'turbo-zzzz');
      await tester.tap(find.text('Send friend request'));
      await tester.pumpAndSettle();

      // The user knows the request actually went out.
      expect(find.text('Request sent'), findsOneWidget);
      // The text field is cleared so they can send another request without
      // backspacing first.
      final tf = tester.widget<TextField>(
          find.widgetWithText(TextField, 'Friend code'));
      expect(tf.controller?.text ?? '', isEmpty,
          reason: 'Successful submit should clear the field.');
      // The right things happened over the wire.
      expect(api.friendCodeLookups.single, 'turbo-zzzz');
      expect(api.friendshipRequests.single,
          '11111111-1111-1111-1111-111111111111');
    });

    testWidgets(
        'unknown code surfaces "No user with that code" — no request fires',
        (tester) async {
      // FakeSharingApi.lookupUserByFriendCode resolves any non-empty code by
      // default. Override the behaviour by clearing it via a stub.
      final api = _NotFoundLookupApi()..myProfile = _profile('ablekite');
      await _goToAddTab(tester, api);

      await tester.enterText(
          find.widgetWithText(TextField, 'Friend code'), 'turbo-doesnotexist');
      await tester.tap(find.text('Send friend request'));
      await tester.pumpAndSettle();

      expect(find.text('No user with that code.'), findsOneWidget,
          reason: 'The user needs a clear explanation when lookup fails — '
              'silent failure leaves them retrying the same broken code.');
      expect(api.friendshipRequests, isEmpty,
          reason: 'No request should fire when lookup returned null.');
    });

    testWidgets('email invite flow: submit, success snackbar, field cleared',
        (tester) async {
      final api = FakeSharingApi()..myProfile = _profile('ablekite');
      await _goToAddTab(tester, api);

      await tester.enterText(
          find.widgetWithText(TextField, 'Email'), 'friend@example.test');
      await tester.ensureVisible(find.text('Send invite'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Send invite'));
      await tester.pumpAndSettle();

      // Confirmation specifically tells the user the invite is delayed
      // (the recipient gets it on signup), not "sent now via email".
      // Match the snackbar exactly to distinguish it from the help text
      // "They'll get the invite when they sign up." rendered alongside.
      expect(
        find.text("Invite sent — they'll join when they sign up"),
        findsOneWidget,
        reason: 'The snackbar should make clear this is an invite-on-signup, '
            'not an immediate notification.',
      );

      final tf = tester.widget<TextField>(
          find.widgetWithText(TextField, 'Email'));
      expect(tf.controller?.text ?? '', isEmpty);
      expect(api.friendInvites.single.email, 'friend@example.test');
    });
  });
}

/// FakeSharingApi variant where the friend-code lookup always returns
/// null, simulating "no such code" on the server.
class _NotFoundLookupApi extends FakeSharingApi {
  @override
  Future<String?> lookupUserByFriendCode(String code) async {
    friendCodeLookups.add(code);
    return null;
  }
}
