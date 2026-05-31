import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:turbo/features/sharing/api.dart';

import 'fakes/fake_sharing_api.dart';

FriendGroup _group({
  String id = 'g1',
  String name = 'Ski crew',
  List<GroupMemberInfo>? members,
}) =>
    FriendGroup(
      id: id,
      ownerId: 'me',
      name: name,
      createdAt: DateTime.utc(2026, 1, 1),
      updatedAt: DateTime.utc(2026, 1, 1),
      members: members ??
          [
            GroupMemberInfo(
                userId: 'me', role: 'admin', joinedAt: DateTime.utc(2026, 1, 1)),
          ],
    );

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  group('GroupsPage — list view', () {
    testWidgets('empty state guides the user toward creating a group',
        (tester) async {
      await tester.pumpWidget(wrapWithSharingFake(const GroupsPage(), FakeSharingApi()));
      await tester.pumpAndSettle();

      expect(find.textContaining('No groups yet'), findsOneWidget);
      expect(find.textContaining('share with several friends'), findsOneWidget,
          reason: 'Empty state must hint at the value of groups — '
              '"create one to share with several friends at once" — '
              'or the feature feels useless to a brand-new user.');
      // The create entry point is visible from the empty state.
      expect(find.text('New group'), findsOneWidget);
    });

    testWidgets('shows each group name with its member-count subtitle',
        (tester) async {
      final api = FakeSharingApi()
        ..groups = [
          _group(name: 'Ski crew', members: [
            GroupMemberInfo(userId: 'me', role: 'admin', joinedAt: DateTime.utc(2026, 1, 1)),
            GroupMemberInfo(userId: 'bob', role: 'member', joinedAt: DateTime.utc(2026, 1, 1)),
          ]),
        ];
      await tester.pumpWidget(wrapWithSharingFake(const GroupsPage(), api));
      await tester.pumpAndSettle();

      expect(find.text('Ski crew'), findsOneWidget);
      expect(find.text('2 members'), findsOneWidget);
    });

    testWidgets('singular "1 member" copy for a one-person group',
        (tester) async {
      final api = FakeSharingApi()
        ..groups = [_group(name: 'Just me')]; // default = 1 admin (me)
      await tester.pumpWidget(wrapWithSharingFake(const GroupsPage(), api));
      await tester.pumpAndSettle();

      expect(find.text('1 member'), findsOneWidget,
          reason: 'Singular/plural matters for polish — "1 members" feels off.');
    });
  });

  group('GroupsPage — creating a group', () {
    testWidgets(
        'New group → enter name → Create → new group appears in the list',
        (tester) async {
      final api = FakeSharingApi();
      await tester.pumpWidget(wrapWithSharingFake(const GroupsPage(), api));
      await tester.pumpAndSettle();

      // Before: empty state visible.
      expect(find.textContaining('No groups yet'), findsOneWidget);

      await tester.tap(find.text('New group'));
      await tester.pumpAndSettle();

      await tester.enterText(
          find.widgetWithText(TextField, 'Group name'), 'Trip group');
      await tester.tap(find.widgetWithText(FilledButton, 'Create'));
      await tester.pumpAndSettle();

      // After: the new group is visible in the list — the user can see
      // that their action took effect without leaving the page.
      expect(find.text('Trip group'), findsOneWidget,
          reason: 'A successful create should update the list. If it does '
              'not, the user is left wondering whether the action worked.');
      expect(find.textContaining('No groups yet'), findsNothing);

      // And the API call was made with the entered name.
      expect(api.groupsCreated.single.name, 'Trip group');
    });

    testWidgets('Cancel in the New group dialog does NOT create anything',
        (tester) async {
      final api = FakeSharingApi();
      await tester.pumpWidget(wrapWithSharingFake(const GroupsPage(), api));
      await tester.pumpAndSettle();

      await tester.tap(find.text('New group'));
      await tester.pumpAndSettle();
      await tester.enterText(
          find.widgetWithText(TextField, 'Group name'), 'Trip group');
      await tester.tap(find.widgetWithText(TextButton, 'Cancel'));
      await tester.pumpAndSettle();

      // Empty state remains; no API call fired.
      expect(find.textContaining('No groups yet'), findsOneWidget);
      expect(api.groupsCreated, isEmpty);
    });

    testWidgets('submitting an empty name does NOT create a group',
        (tester) async {
      final api = FakeSharingApi();
      await tester.pumpWidget(wrapWithSharingFake(const GroupsPage(), api));
      await tester.pumpAndSettle();

      await tester.tap(find.text('New group'));
      await tester.pumpAndSettle();
      // Don't enter anything — just tap Create.
      await tester.tap(find.widgetWithText(FilledButton, 'Create'));
      await tester.pumpAndSettle();

      expect(api.groupsCreated, isEmpty,
          reason: 'Empty name is no-op; the dialog should not spend a server '
              'call on it.');
    });
  });

  group('GroupsPage — opening a group', () {
    testWidgets('tapping a group navigates to its detail page', (tester) async {
      final api = FakeSharingApi()..groups = [_group(name: 'Ski crew')];
      await tester.pumpWidget(wrapWithSharingFake(const GroupsPage(), api));
      await tester.pumpAndSettle();

      await tester.tap(find.text('Ski crew'));
      await tester.pumpAndSettle();

      // Detail page is now visible — Members header is unique to it.
      expect(find.text('Members'), findsOneWidget,
          reason: 'Tapping the row must navigate; without that, the user has '
              'no way to manage who is in the group.');
    });
  });
}
