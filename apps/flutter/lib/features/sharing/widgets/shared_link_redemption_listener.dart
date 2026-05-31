import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:logging/logging.dart';

import '../data/pending_link_redemption_provider.dart';
import '../data/role_cache_repository.dart';
import '../data/sharing_api_client.dart';
import '../models/sharing_models.dart';
import '../providers/sharing_providers.dart';

/// Watches [pendingLinkRedemptionProvider] together with the sharing
/// gate. When the user becomes available (signed in) AND a pending
/// token is queued, calls the server's /links/{token}/redeem endpoint
/// and surfaces a snackbar confirming the access.
///
/// This is the bridge between cold-start `/share/r/<token>` deep links
/// (which arrive before the user signs in) and the server-side
/// materialisation of a per-user grant.
class SharedLinkRedemptionListener extends ConsumerStatefulWidget {
  final Widget child;
  const SharedLinkRedemptionListener({super.key, required this.child});

  @override
  ConsumerState<SharedLinkRedemptionListener> createState() =>
      _SharedLinkRedemptionListenerState();
}

class _SharedLinkRedemptionListenerState
    extends ConsumerState<SharedLinkRedemptionListener> {
  static final _log = Logger('SharedLinkRedemptionListener');
  bool _redeeming = false;

  @override
  Widget build(BuildContext context) {
    final available = ref.watch(sharingAvailableProvider);
    final pendingToken = ref.watch(pendingLinkRedemptionProvider);

    if (available && pendingToken != null && !_redeeming) {
      _redeeming = true;
      WidgetsBinding.instance.addPostFrameCallback((_) => _redeem(pendingToken));
    }

    return widget.child;
  }

  Future<void> _redeem(String token) async {
    try {
      final taken =
          ref.read(pendingLinkRedemptionProvider.notifier).take();
      if (taken == null) return;
      final result =
          await ref.read(sharingApiClientProvider).redeemLink(taken);
      _log.info('Redeemed link for resource ${result.resourceId} (${result.resourceType})');
      // Trigger a sync so the newly-granted resource pulls into the cache.
      try {
        await ref
            .read(roleCacheRepositoryProvider)
            .sync(types: [result.resourceType]);
      } catch (_) {
        // Non-fatal: the next regular sync will catch up.
      }
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(_resultLabel(result))),
        );
      }
    } catch (e, st) {
      _log.warning('Link redemption failed', e, st);
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('That share link is no longer valid.')),
        );
      }
    } finally {
      _redeeming = false;
    }
  }

  String _resultLabel(LinkRedemption result) => switch (result.resourceType) {
        'collection' when result.role == 'editor' =>
          'A collection was shared with you (you can edit).',
        'collection' => 'A collection was shared with you.',
        'marker' when result.role == 'editor' =>
          'A marker was shared with you (you can edit).',
        'marker' => 'A marker was shared with you.',
        'path' when result.role == 'editor' =>
          'A route was shared with you (you can edit).',
        'path' => 'A route was shared with you.',
        _ => 'Shared item added.',
      };
}
