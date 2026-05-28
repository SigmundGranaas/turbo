import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:turbo/features/sharing/api.dart';

import 'fakes/fake_sharing_api.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  group('ShareSheet', () {
    testWidgets('shows the resource title and role selector', (tester) async {
      final api = FakeSharingApi();
      await tester.pumpWidget(wrapWithSharingFake(
        const ShareSheet(resourceId: 'r1', title: 'Trip plan'),
        api,
      ));
      await tester.pumpAndSettle();

      expect(find.text('Trip plan'), findsOneWidget);
      expect(find.text('Can view'), findsOneWidget);
      expect(find.text('Can edit'), findsOneWidget);
    });

    testWidgets('shows accepted friends and grants on tap', (tester) async {
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

      await tester.pumpWidget(wrapWithSharingFake(
        const ShareSheet(resourceId: 'r1', title: 'Trip plan'),
        api,
      ));
      await tester.pumpAndSettle();

      expect(find.text('bob'), findsOneWidget);

      // Tap Share next to bob.
      await tester.tap(find.text('Share').first);
      await tester.pumpAndSettle();

      expect(api.userGrants, hasLength(1));
      expect(api.userGrants.single.userId, 'bob');
      expect(api.userGrants.single.role, GrantRole.viewer);
    });

    testWidgets('changing the role selector to "Can edit" affects subsequent grants',
        (tester) async {
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
      await tester.pumpWidget(wrapWithSharingFake(
        const ShareSheet(resourceId: 'r1', title: 'Trip plan'),
        api,
      ));
      await tester.pumpAndSettle();

      await tester.tap(find.text('Can edit'));
      await tester.pumpAndSettle();

      await tester.tap(find.text('Share').first);
      await tester.pumpAndSettle();

      expect(api.userGrants.single.role, GrantRole.editor);
    });

    testWidgets('shows groups and grants to a group on tap', (tester) async {
      final api = FakeSharingApi()
        ..groups = [
          FriendGroup(
            id: 'g1',
            ownerId: 'me',
            name: 'Ski crew',
            createdAt: DateTime.utc(2026, 1, 1),
            updatedAt: DateTime.utc(2026, 1, 1),
            members: [],
          ),
        ];
      await tester.pumpWidget(wrapWithSharingFake(
        const ShareSheet(resourceId: 'r1', title: 'Trip plan'),
        api,
      ));
      await tester.pumpAndSettle();

      expect(find.text('Ski crew'), findsOneWidget);

      await tester.tap(find.text('Share').first);
      await tester.pumpAndSettle();

      expect(api.groupGrants.single.groupId, 'g1');
    });

    testWidgets('Create link copies a turbo /share/r URL to the clipboard',
        (tester) async {
      String? clipboardText;
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(SystemChannels.platform, (call) async {
        if (call.method == 'Clipboard.setData') {
          clipboardText = (call.arguments as Map)['text'] as String?;
        }
        return null;
      });

      final api = FakeSharingApi();
      await tester.pumpWidget(wrapWithSharingFake(
        const ShareSheet(resourceId: 'r1', title: 'Trip plan'),
        api,
      ));
      await tester.pumpAndSettle();

      await tester.tap(find.text('Create link'));
      await tester.pumpAndSettle();

      expect(api.linkGrants, hasLength(1));
      expect(clipboardText, isNotNull);
      expect(clipboardText!, contains('/share/r/tok-1'));
    });

    testWidgets('Revoke removes the grant from the list', (tester) async {
      final api = FakeSharingApi()
        ..grants = [
          Grant(
            resourceId: 'r1',
            subjectType: 'user',
            subjectId: 'bob',
            role: GrantRole.viewer,
            grantedBy: 'me',
            grantedAt: DateTime.utc(2026, 1, 1),
            expiresAt: null,
          ),
        ];
      await tester.pumpWidget(wrapWithSharingFake(
        const ShareSheet(resourceId: 'r1', title: 'Trip plan'),
        api,
      ));
      await tester.pumpAndSettle();

      // The grant list shows bob under "Existing access".
      expect(find.text('bob'), findsOneWidget);

      // Tap the delete icon next to bob.
      final revokeButton = find.byTooltip('Revoke');
      expect(revokeButton, findsOneWidget);
      await tester.tap(revokeButton);
      await tester.pumpAndSettle();

      expect(api.revocations.single.subjectId, 'bob');
      expect(api.revocations.single.subjectType, 'user');
    });
  });
}

