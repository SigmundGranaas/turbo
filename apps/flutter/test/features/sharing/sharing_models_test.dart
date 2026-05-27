import 'package:flutter_test/flutter_test.dart';
import 'package:turbo/features/sharing/api.dart';

void main() {
  group('ResourceEnvelope', () {
    test('deserializes the sync envelope shape', () {
      final envelope = ResourceEnvelope.fromJson({
        'id': '019e69fb-d730-771c-895c-db965bcc9890',
        'type': 'collection',
        'ownerId': 'aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa',
        'visibility': 'private',
        'myRole': 'editor',
        'version': 3,
        'updatedAt': '2026-05-27T10:11:12.000Z',
        'deleted': false,
      });

      expect(envelope.id, '019e69fb-d730-771c-895c-db965bcc9890');
      expect(envelope.type, 'collection');
      expect(envelope.visibility, ResourceVisibility.private);
      expect(envelope.myRole, EffectiveRole.editor);
      expect(envelope.version, 3);
      expect(envelope.deleted, isFalse);
    });

    test('omitted deleted defaults to false', () {
      final envelope = ResourceEnvelope.fromJson({
        'id': 'x',
        'type': 'marker',
        'ownerId': 'y',
        'visibility': 'public',
        'myRole': 'owner',
        'version': 1,
        'updatedAt': '2026-05-27T10:11:12.000Z',
      });
      expect(envelope.deleted, isFalse);
    });
  });

  group('EffectiveRole', () {
    test('canEdit is true for editor and owner only', () {
      expect(EffectiveRole.viewer.canEdit, isFalse);
      expect(EffectiveRole.editor.canEdit, isTrue);
      expect(EffectiveRole.owner.canEdit, isTrue);
    });

    test('isOwner is owner-only', () {
      expect(EffectiveRole.viewer.isOwner, isFalse);
      expect(EffectiveRole.editor.isOwner, isFalse);
      expect(EffectiveRole.owner.isOwner, isTrue);
    });
  });

  group('Friendship', () {
    test('parses a pending friendship without acceptedAt', () {
      final f = Friendship.fromJson({
        'otherUserId': 'bob',
        'initiatorId': 'alice',
        'status': 'pending',
        'createdAt': '2026-05-27T10:00:00.000Z',
        'acceptedAt': null,
      });
      expect(f.status, FriendshipStatus.pending);
      expect(f.acceptedAt, isNull);
    });

    test('parses an accepted friendship', () {
      final f = Friendship.fromJson({
        'otherUserId': 'bob',
        'initiatorId': 'alice',
        'status': 'accepted',
        'createdAt': '2026-05-27T10:00:00.000Z',
        'acceptedAt': '2026-05-27T10:05:00.000Z',
      });
      expect(f.status, FriendshipStatus.accepted);
      expect(f.acceptedAt, isNotNull);
    });
  });

  group('FriendGroup', () {
    test('parses a group with members', () {
      final g = FriendGroup.fromJson({
        'id': 'g1',
        'ownerId': 'alice',
        'name': 'Ski crew',
        'createdAt': '2026-05-27T10:00:00.000Z',
        'updatedAt': '2026-05-27T10:00:00.000Z',
        'members': [
          {
            'userId': 'alice',
            'role': 'admin',
            'joinedAt': '2026-05-27T10:00:00.000Z',
          },
          {
            'userId': 'bob',
            'role': 'member',
            'joinedAt': '2026-05-27T10:01:00.000Z',
          },
        ],
      });
      expect(g.name, 'Ski crew');
      expect(g.members, hasLength(2));
      expect(g.members.first.role, 'admin');
    });
  });

  group('LinkGrant', () {
    test('carries the link token', () {
      final lg = LinkGrant.fromJson({
        'resourceId': 'r1',
        'subjectId': 's1',
        'linkToken': 'abc123',
        'role': 'viewer',
        'grantedAt': '2026-05-27T10:00:00.000Z',
        'expiresAt': null,
      });
      expect(lg.linkToken, 'abc123');
      expect(lg.role, GrantRole.viewer);
    });
  });
}
