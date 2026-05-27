import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../data/sharing_api_client.dart';
import '../models/sharing_models.dart';
import '../providers/sharing_providers.dart';

/// Manages the user's friend graph. Three tabs:
/// - Friends: accepted friendships, with "remove" affordance.
/// - Pending: requests sent or received, with accept/decline.
/// - Add: send a friend request by user id, or invite by email if the
///   recipient isn't on the platform yet.
class FriendsPage extends ConsumerStatefulWidget {
  const FriendsPage({super.key});

  @override
  ConsumerState<FriendsPage> createState() => _FriendsPageState();
}

class _FriendsPageState extends ConsumerState<FriendsPage> {
  @override
  Widget build(BuildContext context) {
    return DefaultTabController(
      length: 3,
      child: Scaffold(
        appBar: AppBar(
          title: const Text('Friends'),
          bottom: const TabBar(tabs: [
            Tab(text: 'Friends'),
            Tab(text: 'Pending'),
            Tab(text: 'Add'),
          ]),
        ),
        body: TabBarView(
          children: [
            _FriendsListTab(onChanged: () => ref.invalidate(allFriendshipsProvider)),
            _PendingTab(onChanged: () => ref.invalidate(allFriendshipsProvider)),
            _AddTab(onSent: () => ref.invalidate(allFriendshipsProvider)),
          ],
        ),
      ),
    );
  }
}

class _FriendsListTab extends ConsumerWidget {
  final VoidCallback onChanged;
  const _FriendsListTab({required this.onChanged});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final friends = ref.watch(acceptedFriendsProvider);
    return friends.when(
      loading: () => const Center(child: CircularProgressIndicator()),
      error: (e, _) => Center(child: Text('Failed to load: $e')),
      data: (list) {
        if (list.isEmpty) return const Center(child: Text('No friends yet.'));
        return ListView.builder(
          itemCount: list.length,
          itemBuilder: (_, i) => ListTile(
            leading: const Icon(Icons.person_outline),
            title: Text(list[i].otherUserId),
            subtitle: Text('Since ${list[i].acceptedAt?.toLocal().toString() ?? '—'}'),
            trailing: IconButton(
              icon: const Icon(Icons.person_remove_outlined),
              tooltip: 'Remove',
              onPressed: () async {
                await ref.read(sharingApiClientProvider).removeFriendship(list[i].otherUserId);
                ref.invalidate(acceptedFriendsProvider);
                onChanged();
              },
            ),
          ),
        );
      },
    );
  }
}

class _PendingTab extends ConsumerWidget {
  final VoidCallback onChanged;
  const _PendingTab({required this.onChanged});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final all = ref.watch(allFriendshipsProvider);
    return all.when(
      loading: () => const Center(child: CircularProgressIndicator()),
      error: (e, _) => Center(child: Text('Failed to load: $e')),
      data: (list) {
        final pending =
            list.where((f) => f.status == FriendshipStatus.pending).toList();
        if (pending.isEmpty) return const Center(child: Text('No pending requests.'));
        return ListView.builder(
          itemCount: pending.length,
          itemBuilder: (_, i) {
            final f = pending[i];
            // Note: we don't know our own user id here without an auth lookup.
            // For now, surface initiator vs other and let the user accept either way.
            return ListTile(
              leading: const Icon(Icons.hourglass_top_outlined),
              title: Text(f.otherUserId),
              subtitle: Text('Initiator: ${f.initiatorId}'),
              trailing: Wrap(
                spacing: 4,
                children: [
                  IconButton(
                    icon: const Icon(Icons.check),
                    tooltip: 'Accept',
                    onPressed: () async {
                      try {
                        await ref.read(sharingApiClientProvider).acceptFriendship(f.otherUserId);
                        ref.invalidate(allFriendshipsProvider);
                        ref.invalidate(acceptedFriendsProvider);
                      } catch (e) {
                        if (context.mounted) {
                          ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('$e')));
                        }
                      }
                      onChanged();
                    },
                  ),
                  IconButton(
                    icon: const Icon(Icons.close),
                    tooltip: 'Decline',
                    onPressed: () async {
                      await ref.read(sharingApiClientProvider).removeFriendship(f.otherUserId);
                      ref.invalidate(allFriendshipsProvider);
                      onChanged();
                    },
                  ),
                ],
              ),
            );
          },
        );
      },
    );
  }
}

class _AddTab extends ConsumerStatefulWidget {
  final VoidCallback onSent;
  const _AddTab({required this.onSent});

  @override
  ConsumerState<_AddTab> createState() => _AddTabState();
}

class _AddTabState extends ConsumerState<_AddTab> {
  final _userIdController = TextEditingController();
  final _emailController = TextEditingController();
  bool _busy = false;

  @override
  void dispose() {
    _userIdController.dispose();
    _emailController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.all(16),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const Text('Add a friend already on the platform',
              style: TextStyle(fontWeight: FontWeight.bold)),
          const SizedBox(height: 8),
          TextField(
            controller: _userIdController,
            decoration: const InputDecoration(
              labelText: 'User id',
              border: OutlineInputBorder(),
            ),
          ),
          const SizedBox(height: 8),
          FilledButton(
            onPressed: _busy ? null : _sendRequest,
            child: const Text('Send friend request'),
          ),
          const SizedBox(height: 24),
          const Divider(),
          const SizedBox(height: 16),
          const Text('Invite someone by email',
              style: TextStyle(fontWeight: FontWeight.bold)),
          const SizedBox(height: 8),
          TextField(
            controller: _emailController,
            decoration: const InputDecoration(
              labelText: 'Email',
              border: OutlineInputBorder(),
            ),
          ),
          const SizedBox(height: 8),
          FilledButton.tonal(
            onPressed: _busy ? null : _sendInvite,
            child: const Text('Send invite'),
          ),
        ],
      ),
    );
  }

  Future<void> _sendRequest() async {
    final text = _userIdController.text.trim();
    if (text.isEmpty) return;
    setState(() => _busy = true);
    try {
      await ref.read(sharingApiClientProvider).requestFriendship(text);
      _userIdController.clear();
      widget.onSent();
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(const SnackBar(content: Text('Request sent')));
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('Failed: $e')));
      }
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  Future<void> _sendInvite() async {
    final text = _emailController.text.trim();
    if (text.isEmpty) return;
    setState(() => _busy = true);
    try {
      await ref.read(sharingApiClientProvider).createFriendInvite(text);
      _emailController.clear();
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Invite sent — they\'ll join when they sign up')),
        );
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text('Failed: $e')));
      }
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }
}
