import 'package:flutter_riverpod/flutter_riverpod.dart';

/// Download IDs the user has dismissed from the in-progress download toolbar.
///
/// Lifted out of `MainMapPage`'s local state so the download toolbar can be a
/// self-contained map-overlay descriptor (it reads this + the regions list and
/// renders itself, with its hide button writing here).
class HiddenDownloadsNotifier extends Notifier<Set<String>> {
  @override
  Set<String> build() => const {};

  void hide(String id) => state = {...state, id};
}

final hiddenDownloadsProvider =
    NotifierProvider<HiddenDownloadsNotifier, Set<String>>(
  HiddenDownloadsNotifier.new,
);
