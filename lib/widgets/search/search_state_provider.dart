import 'dart:async';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/data/search/composite_search_service.dart';
import 'package:turbo/data/search/location_service.dart';

// The Notifier for our search logic
class SearchNotifier extends AutoDisposeAsyncNotifier<List<LocationSearchResult>> {
  Timer? _debounce;

  @override
  Future<List<LocationSearchResult>> build() async {
    // On dispose, cancel any active debounce timer
    ref.onDispose(() => _debounce?.cancel());
    // The initial state is an empty list of results.
    return [];
  }

  Future<void> search(String query) async {
    _debounce?.cancel();

    if (query.trim().length < 2) {
      // If the query is too short, clear results and stop.
      state = const AsyncData([]);
      return;
    }

    // Set the state to loading immediately.
    state = const AsyncLoading();

    _debounce = Timer(const Duration(milliseconds: 400), () async {
      final searchService = ref.read(compositeSearchServiceProvider);
      try {
        final data = await searchService.findLocationsBy(query);
        state = AsyncData(data);
      } catch (e, st) {
        // Same for error state.
        state = AsyncError(e, st);
      }
    });
  }

  // A method to reset the state, e.g., when a suggestion is picked.
  void clear() {
    state = const AsyncData([]);
  }
}

// The provider itself
final searchProvider =
AutoDisposeAsyncNotifierProvider<SearchNotifier, List<LocationSearchResult>>(
  SearchNotifier.new,
);