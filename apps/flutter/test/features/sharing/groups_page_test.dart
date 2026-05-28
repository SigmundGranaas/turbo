import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:turbo/features/sharing/api.dart';

import 'fakes/fake_sharing_api.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  group('GroupsPage', () {
    testWidgets('empty state when no groups exist', (tester) async {
      final api = FakeSharingApi();
      await tester.pumpWidget(wrapWithSharingFake(const GroupsPage(), api));
      await tester.pumpAndSettle();

      expect(find.textContaining('No groups yet'), findsOneWidget);
    });

    testWidgets('lists existing groups with member counts', (tester) async {
      final api = FakeSharingApi()
        ..groups = [
          FriendGroup(
            id: 'g1',
            ownerId: 'me',
            name: 'Ski crew',
            createdAt: DateTime.utc(2026, 1, 1),
            updatedAt: DateTime.utc(2026, 1, 1),
            members: [
              GroupMemberInfo(userId: 'me', role: 'admin', joinedAt: DateTime.utc(2026, 1, 1)),
              GroupMemberInfo(userId: 'bob', role: 'member', joinedAt: DateTime.utc(2026, 1, 1)),
            ],
          ),
        ];
      await tester.pumpWidget(wrapWithSharingFake(const GroupsPage(), api));
      await tester.pumpAndSettle();

      expect(find.text('Ski crew'), findsOneWidget);
      expect(find.text('2 members'), findsOneWidget);
    });

    testWidgets('tapping "New group" and submitting creates a group', (tester) async {
      final api = FakeSharingApi();
      await tester.pumpWidget(wrapWithSharingFake(const GroupsPage(), api));
      await tester.pumpAndSettle();

      await tester.tap(find.text('New group'));
      await tester.pumpAndSettle();

      // The dialog has a "Group name" TextField.
      await tester.enterText(
          find.widgetWithText(TextField, 'Group name'), 'Trip group');
      await tester.tap(find.widgetWithText(FilledButton, 'Create'));
      await tester.pumpAndSettle();

      expect(api.groupsCreated.single.name, 'Trip group');
    });

    testWidgets('singular "1 member" label for a group of one', (tester) async {
      final api = FakeSharingApi()
        ..groups = [
          FriendGroup(
            id: 'g1',
            ownerId: 'me',
            name: 'Just me',
            createdAt: DateTime.utc(2026, 1, 1),
            updatedAt: DateTime.utc(2026, 1, 1),
            members: [
              GroupMemberInfo(userId: 'me', role: 'admin', joinedAt: DateTime.utc(2026, 1, 1)),
            ],
          ),
        ];
      await tester.pumpWidget(wrapWithSharingFake(const GroupsPage(), api));
      await tester.pumpAndSettle();

      expect(find.text('1 member'), findsOneWidget);
    });
  });
}
