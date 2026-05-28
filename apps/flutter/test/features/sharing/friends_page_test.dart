import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:turbo/features/sharing/api.dart';

import 'fakes/fake_sharing_api.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  group('FriendsPage', () {
    testWidgets('Friends tab lists accepted friends', (tester) async {
      final api = FakeSharingApi()
        ..acceptedFriends = [
          Friendship(
            otherUserId: 'bob',
            initiatorId: 'me',
            status: FriendshipStatus.accepted,
            createdAt: DateTime.utc(2026, 1, 1),
            acceptedAt: DateTime.utc(2026, 1, 2),
          ),
        ];
      await tester.pumpWidget(wrapWithSharingFake(const FriendsPage(), api));
      await tester.pumpAndSettle();

      expect(find.text('Friends'), findsAtLeastNWidgets(1));
      expect(find.text('bob'), findsOneWidget);
    });

    testWidgets('Pending tab shows accept and decline buttons', (tester) async {
      final api = FakeSharingApi()
        ..allFriendships = [
          Friendship(
            otherUserId: 'alice',
            initiatorId: 'alice',
            status: FriendshipStatus.pending,
            createdAt: DateTime.utc(2026, 1, 1),
            acceptedAt: null,
          ),
        ];
      await tester.pumpWidget(wrapWithSharingFake(const FriendsPage(), api));
      await tester.pumpAndSettle();

      await tester.tap(find.text('Pending'));
      await tester.pumpAndSettle();

      expect(find.text('alice'), findsOneWidget);

      await tester.tap(find.byTooltip('Accept'));
      await tester.pumpAndSettle();

      expect(api.friendshipAccepts.single, 'alice');
    });

    testWidgets('Add tab shows the user\'s own friend code with a copy button',
        (tester) async {
      final api = FakeSharingApi()
        ..myProfile = UserProfile(
          userId: 'me',
          friendCode: 'ablekite',
          createdAt: DateTime.utc(2026, 1, 1),
        );
      await tester.pumpWidget(wrapWithSharingFake(const FriendsPage(), api));
      await tester.pumpAndSettle();

      await tester.tap(find.text('Add'));
      await tester.pumpAndSettle();

      // SelectableText renders both an EditableText and an internal label;
      // assert at least one is present rather than pinning to "exactly one".
      expect(find.text('turbo-ablekite'), findsAtLeastNWidgets(1));
      expect(find.byTooltip('Copy'), findsOneWidget);
    });

    testWidgets('Add tab sends a friend request via friend-code lookup',
        (tester) async {
      final api = FakeSharingApi()
        ..myProfile = UserProfile(
          userId: 'me',
          friendCode: 'ablekite',
          createdAt: DateTime.utc(2026, 1, 1),
        );
      await tester.pumpWidget(wrapWithSharingFake(const FriendsPage(), api));
      await tester.pumpAndSettle();

      await tester.tap(find.text('Add'));
      await tester.pumpAndSettle();

      await tester.enterText(
          find.widgetWithText(TextField, 'Friend code'), 'turbo-zzzz');
      await tester.tap(find.text('Send friend request'));
      await tester.pumpAndSettle();

      expect(api.friendCodeLookups.single, 'turbo-zzzz');
      expect(api.friendshipRequests.single,
          '11111111-1111-1111-1111-111111111111');
    });

    testWidgets('Add tab sends an email invite', (tester) async {
      final api = FakeSharingApi()
        ..myProfile = UserProfile(
          userId: 'me',
          friendCode: 'ablekite',
          createdAt: DateTime.utc(2026, 1, 1),
        );
      await tester.pumpWidget(wrapWithSharingFake(const FriendsPage(), api));
      await tester.pumpAndSettle();

      await tester.tap(find.text('Add'));
      await tester.pumpAndSettle();

      await tester.enterText(
          find.widgetWithText(TextField, 'Email'), 'friend@example.test');
      await tester.ensureVisible(find.text('Send invite'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Send invite'));
      await tester.pumpAndSettle();

      expect(api.friendInvites.single.email, 'friend@example.test');
    });
  });
}
