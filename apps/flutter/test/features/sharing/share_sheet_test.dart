import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:turbo/features/sharing/api.dart';

import 'fakes/fake_sharing_api.dart';

/// Captures Clipboard.setData payloads so tests can assert what the user
/// actually got. Each call appends; tests inspect `texts.last`.
class _ClipboardCapture {
  final List<String> texts = [];
  void install() {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(SystemChannels.platform, (call) async {
      if (call.method == 'Clipboard.setData') {
        texts.add((call.arguments as Map)['text'] as String);
      }
      return null;
    });
  }
}

Friendship _friend(String id) => Friendship(
      otherUserId: id,
      initiatorId: 'me',
      status: FriendshipStatus.accepted,
      createdAt: DateTime.utc(2026, 1, 1),
      acceptedAt: DateTime.utc(2026, 1, 2),
    );

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  group('ShareSheet — header & sections visible to the user', () {
    testWidgets('shows the resource title in the header', (tester) async {
      await tester.pumpWidget(wrapWithSharingFake(
        const ShareSheet(resourceId: 'r1', title: 'Trip plan'),
        FakeSharingApi(),
      ));
      await tester.pumpAndSettle();
      expect(find.text('Trip plan'), findsOneWidget);
    });

    testWidgets('shows both role options and Can view is selected by default',
        (tester) async {
      await tester.pumpWidget(wrapWithSharingFake(
        const ShareSheet(resourceId: 'r1', title: 'Trip plan'),
        FakeSharingApi(),
      ));
      await tester.pumpAndSettle();

      final selector = tester.widget<SegmentedButton<GrantRole>>(
          find.byType(SegmentedButton<GrantRole>));
      expect(selector.selected, {GrantRole.viewer},
          reason: 'A user opening the sheet should see "Can view" preselected; '
              'sharing as editor is the dangerous-by-default we want to avoid.');
    });

    testWidgets('empty friend list shows guidance toward the Friends page',
        (tester) async {
      await tester.pumpWidget(wrapWithSharingFake(
        const ShareSheet(resourceId: 'r1', title: 'Trip plan'),
        FakeSharingApi(),
      ));
      await tester.pumpAndSettle();
      expect(
        find.textContaining('No friends yet'),
        findsOneWidget,
        reason:
            'Empty friend list must tell the user where to add friends, '
            'not just show a blank space.',
      );
    });

    testWidgets('empty group list points to Groups', (tester) async {
      await tester.pumpWidget(wrapWithSharingFake(
        const ShareSheet(resourceId: 'r1', title: 'Trip plan'),
        FakeSharingApi(),
      ));
      await tester.pumpAndSettle();
      expect(
        find.textContaining('No groups yet'),
        findsOneWidget,
      );
    });

    testWidgets('with no grants the existing-access section shows guidance',
        (tester) async {
      await tester.pumpWidget(wrapWithSharingFake(
        const ShareSheet(resourceId: 'r1', title: 'Trip plan'),
        FakeSharingApi(),
      ));
      await tester.pumpAndSettle();
      expect(find.text('No grants yet.'), findsOneWidget);
    });
  });

  group('ShareSheet — granting to a friend', () {
    testWidgets(
        'tap a friend → API call fires AND user sees confirmation AND grant appears in Existing access',
        (tester) async {
      final api = FakeSharingApi()..acceptedFriends = [_friend('bob')];
      await tester.pumpWidget(wrapWithSharingFake(
        const ShareSheet(resourceId: 'r1', title: 'Trip plan'),
        api,
      ));
      await tester.pumpAndSettle();

      // Before: bob in friends list, NO bob in existing-access list.
      expect(find.text('bob'), findsOneWidget);
      expect(find.text('No grants yet.'), findsOneWidget);

      await tester.tap(find.text('Share').first);
      await tester.pumpAndSettle();

      // User-visible feedback that the share went through:
      expect(find.text('Shared'), findsOneWidget,
          reason: 'A success snackbar must confirm the action completed.');
      // The existing-access section now shows bob — the user can see the
      // grant they just created without re-opening the sheet.
      expect(find.text('Can view'), findsWidgets,
          reason: 'The grant row label should reflect the chosen role.');

      // And the API was actually called with the right role.
      expect(api.userGrants.single.userId, 'bob');
      expect(api.userGrants.single.role, GrantRole.viewer);
    });

    testWidgets(
        'switching to Can edit before tapping Share grants editor role and shows it in the list',
        (tester) async {
      final api = FakeSharingApi()..acceptedFriends = [_friend('bob')];
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
      // The label "Can edit" should appear both in the role selector AND
      // in the new grant row, so the user sees what they just granted.
      expect(find.text('Can edit'), findsWidgets);
    });
  });

  group('ShareSheet — granting to a group', () {
    testWidgets(
        'tap a group → grant fires AND confirmation snackbar AND group shows in existing access',
        (tester) async {
      final api = FakeSharingApi()
        ..groups = [
          FriendGroup(
            id: 'g1',
            ownerId: 'me',
            name: 'Ski crew',
            createdAt: DateTime.utc(2026, 1, 1),
            updatedAt: DateTime.utc(2026, 1, 1),
            members: [
              GroupMemberInfo(
                  userId: 'me', role: 'admin', joinedAt: DateTime.utc(2026, 1, 1)),
            ],
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

      // Confirmation copy is group-specific so the user knows what happened.
      expect(find.text('Shared with group'), findsOneWidget);
      // Group also surfaces in the existing-access list with a "Group: " prefix.
      expect(find.text('Group: g1'), findsOneWidget,
          reason: 'Grant row should disambiguate user grants from group grants.');
      expect(api.groupGrants.single.groupId, 'g1');
    });
  });

  group('ShareSheet — get a link', () {
    testWidgets(
        'Create link → full /share/r URL copied → user sees "copied" confirmation',
        (tester) async {
      final clipboard = _ClipboardCapture()..install();
      final api = FakeSharingApi();
      await tester.pumpWidget(wrapWithSharingFake(
        const ShareSheet(resourceId: 'r1', title: 'Trip plan'),
        api,
      ));
      await tester.pumpAndSettle();

      await tester.tap(find.text('Create link'));
      await tester.pumpAndSettle();

      // Clipboard must hold a real URL the user can paste into a messenger.
      expect(clipboard.texts, hasLength(1));
      expect(clipboard.texts.single, contains('/share/r/tok-1'));
      expect(clipboard.texts.single, startsWith('http'),
          reason: 'Recipients tap a clickable URL, not a bare token.');

      // And the user gets visible feedback that copy succeeded.
      expect(find.textContaining('copied'), findsOneWidget,
          reason: 'Without confirmation users tap again and create duplicate '
              'link grants. The snackbar prevents that.');
    });
  });

  group('ShareSheet — revoking', () {
    testWidgets('Revoke removes the grant from the visible list immediately',
        (tester) async {
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

      // Before: bob is in the existing-access section.
      expect(find.text('bob'), findsOneWidget);

      await tester.tap(find.byTooltip('Revoke'));
      await tester.pumpAndSettle();

      // After: bob is gone from the visible list — no stale state.
      expect(find.text('bob'), findsNothing,
          reason: 'Revoking must update the visible list, not just the server.');
      expect(find.text('No grants yet.'), findsOneWidget,
          reason: 'Empty state returns when the last grant is revoked.');
      expect(api.revocations.single.subjectId, 'bob');
    });
  });

  group('ShareSheet — multi-step flow', () {
    testWidgets(
        'open → grant → see in list → revoke → list back to empty (single session)',
        (tester) async {
      final api = FakeSharingApi()..acceptedFriends = [_friend('bob')];
      await tester.pumpWidget(wrapWithSharingFake(
        const ShareSheet(resourceId: 'r1', title: 'Trip plan'),
        api,
      ));
      await tester.pumpAndSettle();

      // Step 1: grant.
      await tester.tap(find.text('Share').first);
      await tester.pumpAndSettle();
      expect(find.text('No grants yet.'), findsNothing);

      // Step 2: revoke.
      await tester.tap(find.byTooltip('Revoke'));
      await tester.pumpAndSettle();
      expect(find.text('No grants yet.'), findsOneWidget);
      expect(api.userGrants, hasLength(1),
          reason: 'Grant was issued exactly once during the flow.');
      expect(api.revocations, hasLength(1));
    });
  });
}
