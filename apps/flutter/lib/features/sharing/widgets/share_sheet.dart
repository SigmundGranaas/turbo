import 'package:flutter/material.dart';
import 'package:turbo/core/widgets/exclusive_sheet.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/core/config/env_config.dart';

import '../data/sharing_api_client.dart';
import '../models/sharing_models.dart';
import '../providers/sharing_providers.dart';

/// Universal share sheet. Works for any resource id (collection,
/// marker, path, or any future shareable type) — the sheet has no
/// compile-time knowledge of what the payload is.
///
/// Sections:
/// - "Share with a friend" (autocomplete from accepted friendships)
/// - "Share with a group"
/// - "Get a link" (creates a tracked link grant, copies to clipboard)
/// - "Existing access" (list of current grants with revoke buttons)
///
/// Only the resource owner can issue or revoke grants; non-owners see
/// the existing-access list as read-only.
class ShareSheet extends ConsumerStatefulWidget {
  final String resourceId;
  final String? title;

  const ShareSheet({super.key, required this.resourceId, this.title});

  static Future<void> show(BuildContext context, String resourceId, {String? title}) {
    return showExclusiveSheet<void>(
      context,
      replace: false,
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(top: Radius.circular(16)),
      ),
      builder: (_) => ShareSheet(resourceId: resourceId, title: title),
    );
  }

  @override
  ConsumerState<ShareSheet> createState() => _ShareSheetState();
}

class _ShareSheetState extends ConsumerState<ShareSheet> {
  GrantRole _role = GrantRole.viewer;
  bool _busy = false;

  @override
  Widget build(BuildContext context) {
    final role = ref.watch(effectiveRoleProvider(widget.resourceId));
    final isOwner = role?.isOwner ?? true; // implicit-owner fallback for unknown resources
    final friends = ref.watch(acceptedFriendsProvider);
    final grants = ref.watch(grantsForResourceProvider(widget.resourceId));

    return SafeArea(
      child: SingleChildScrollView(
        padding: EdgeInsets.only(
          left: 16,
          right: 16,
          top: 16,
          bottom: MediaQuery.of(context).viewInsets.bottom + 16,
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          mainAxisSize: MainAxisSize.min,
          children: [
            Row(children: [
              Expanded(
                child: Text(
                  widget.title ?? 'Share',
                  style: Theme.of(context).textTheme.titleLarge,
                ),
              ),
              IconButton(
                icon: const Icon(Icons.close),
                onPressed: () => Navigator.of(context).pop(),
              ),
            ]),
            if (!isOwner)
              const Padding(
                padding: EdgeInsets.only(top: 8),
                child: Text(
                  'Only the owner can share this. Showing current access only.',
                ),
              ),
            if (isOwner) ...[
              const SizedBox(height: 8),
              _RoleSelector(
                role: _role,
                onChanged: (r) => setState(() => _role = r),
              ),
              const SizedBox(height: 16),
              const Text('Share with a friend', style: TextStyle(fontWeight: FontWeight.bold)),
              const SizedBox(height: 8),
              friends.when(
                loading: () => const LinearProgressIndicator(),
                error: (e, _) => Text('Failed to load friends: $e'),
                data: (list) => list.isEmpty
                    ? const Text('No friends yet. Send a friend invite from Friends.')
                    : Column(
                        children: list
                            .map((f) => ListTile(
                                  dense: true,
                                  leading: const Icon(Icons.person_outline),
                                  title: Text(f.otherUserId),
                                  trailing: TextButton(
                                    onPressed: _busy ? null : () => _grantToUser(f.otherUserId),
                                    child: const Text('Share'),
                                  ),
                                ))
                            .toList(),
                      ),
              ),
              const SizedBox(height: 16),
              const Text('Share with a group', style: TextStyle(fontWeight: FontWeight.bold)),
              const SizedBox(height: 8),
              Consumer(builder: (ctx, ref, _) {
                final groups = ref.watch(myGroupsProvider);
                return groups.when(
                  loading: () => const LinearProgressIndicator(),
                  error: (e, _) => Text('Failed to load groups: $e'),
                  data: (list) => list.isEmpty
                      ? const Text('No groups yet. Create one from Groups.')
                      : Column(
                          children: list
                              .map((g) => ListTile(
                                    dense: true,
                                    leading: const Icon(Icons.group_outlined),
                                    title: Text(g.name),
                                    subtitle: Text('${g.members.length} member${g.members.length == 1 ? '' : 's'}'),
                                    trailing: TextButton(
                                      onPressed: _busy ? null : () => _grantToGroup(g.id),
                                      child: const Text('Share'),
                                    ),
                                  ))
                              .toList(),
                        ),
                );
              }),
              const SizedBox(height: 16),
              const Text('Or get a link', style: TextStyle(fontWeight: FontWeight.bold)),
              const SizedBox(height: 8),
              FilledButton.icon(
                icon: const Icon(Icons.link),
                onPressed: _busy ? null : _createLink,
                label: const Text('Create link'),
              ),
            ],
            const Divider(height: 32),
            const Text('Existing access', style: TextStyle(fontWeight: FontWeight.bold)),
            const SizedBox(height: 8),
            grants.when(
              loading: () => const LinearProgressIndicator(),
              error: (e, _) => Text('Failed to load grants: $e'),
              data: (list) => list.isEmpty
                  ? const Text('No grants yet.')
                  : Column(
                      children: list.map((g) => _GrantRow(
                            grant: g,
                            canRevoke: isOwner,
                            onRevoke: () => _revoke(g),
                          )).toList(),
                    ),
            ),
          ],
        ),
      ),
    );
  }

  Future<void> _grantToUser(String userId) async {
    setState(() => _busy = true);
    try {
      await ref.read(sharingApiClientProvider).grantToUser(widget.resourceId, userId, _role);
      ref.invalidate(grantsForResourceProvider(widget.resourceId));
      if (mounted) ScaffoldMessenger.of(context).showSnackBar(const SnackBar(content: Text('Shared')));
    } catch (e) {
      if (mounted) ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('Failed to share: $e')));
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  Future<void> _grantToGroup(String groupId) async {
    setState(() => _busy = true);
    try {
      await ref.read(sharingApiClientProvider).grantToGroup(widget.resourceId, groupId, _role);
      ref.invalidate(grantsForResourceProvider(widget.resourceId));
      if (mounted) ScaffoldMessenger.of(context).showSnackBar(const SnackBar(content: Text('Shared with group')));
    } catch (e) {
      if (mounted) ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('Failed to share: $e')));
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  Future<void> _createLink() async {
    setState(() => _busy = true);
    final messenger = ScaffoldMessenger.of(context);
    try {
      final link = await ref.read(sharingApiClientProvider).grantAsLink(widget.resourceId, _role);
      ref.invalidate(grantsForResourceProvider(widget.resourceId));
      if (mounted) {
        final url = '${EnvironmentConfig.webBaseUrl}/share/r/${link.linkToken}';
        await Clipboard.setData(ClipboardData(text: url));
        messenger.showSnackBar(
          const SnackBar(content: Text('Share link copied to clipboard')),
        );
      }
    } catch (e) {
      if (mounted) messenger.showSnackBar(SnackBar(content: Text('Failed: $e')));
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  Future<void> _revoke(Grant g) async {
    final api = ref.read(sharingApiClientProvider);
    try {
      switch (g.subjectType) {
        case 'user':
          await api.revokeUserGrant(g.resourceId, g.subjectId);
          break;
        case 'group':
          await api.revokeGroupGrant(g.resourceId, g.subjectId);
          break;
        case 'link':
          await api.revokeLinkGrant(g.resourceId, g.subjectId);
          break;
      }
      ref.invalidate(grantsForResourceProvider(widget.resourceId));
    } catch (e) {
      if (mounted) ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('Failed to revoke: $e')));
    }
  }
}

class _RoleSelector extends StatelessWidget {
  final GrantRole role;
  final ValueChanged<GrantRole> onChanged;
  const _RoleSelector({required this.role, required this.onChanged});

  @override
  Widget build(BuildContext context) {
    return SegmentedButton<GrantRole>(
      segments: const [
        ButtonSegment(value: GrantRole.viewer, label: Text('Can view'), icon: Icon(Icons.visibility_outlined)),
        ButtonSegment(value: GrantRole.editor, label: Text('Can edit'), icon: Icon(Icons.edit_outlined)),
      ],
      selected: {role},
      onSelectionChanged: (s) => onChanged(s.first),
    );
  }
}

class _GrantRow extends StatelessWidget {
  final Grant grant;
  final bool canRevoke;
  final VoidCallback onRevoke;
  const _GrantRow({required this.grant, required this.canRevoke, required this.onRevoke});

  @override
  Widget build(BuildContext context) {
    final icon = switch (grant.subjectType) {
      'user' => Icons.person_outline,
      'group' => Icons.group_outlined,
      'link' => Icons.link,
      _ => Icons.help_outline,
    };
    final label = switch (grant.subjectType) {
      'user' => grant.subjectId,
      'group' => 'Group: ${grant.subjectId}',
      'link' => 'Link',
      _ => grant.subjectId,
    };
    return ListTile(
      dense: true,
      leading: Icon(icon),
      title: Text(label),
      subtitle: Text(grant.role == GrantRole.editor ? 'Can edit' : 'Can view'),
      trailing: canRevoke
          ? IconButton(
              icon: const Icon(Icons.delete_outline),
              tooltip: 'Revoke',
              onPressed: onRevoke,
            )
          : null,
    );
  }
}
