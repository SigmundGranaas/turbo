import 'dart:async';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:map_app/data/search/location_service.dart';

final searchLogicProvider = StateNotifierProvider.autoDispose<SearchNotifier, SearchState>((ref) {
  // The service will be passed to the notifier's methods from the UI.
  return SearchNotifier(ref);
});

class SearchState {
  final List<LocationSearchResult> suggestions;
  final bool isLoading;
  final bool isFocused;

  const SearchState({
    this.suggestions = const [],
    this.isLoading = false,
    this.isFocused = false,
  });

  SearchState copyWith({
    List<LocationSearchResult>? suggestions,
    bool? isLoading,
    bool? isFocused,
  }) {
    return SearchState(
      suggestions: suggestions ?? this.suggestions,
      isLoading: isLoading ?? this.isLoading,
      isFocused: isFocused ?? this.isFocused,
    );
  }
}

class SearchNotifier extends StateNotifier<SearchState> {
  final Ref _ref;
  Timer? _debounce;

  SearchNotifier(this._ref) : super(const SearchState());

  void onSearchChanged(String query, LocationService service) {
    if (query.length < 2) {
      if (mounted) state = state.copyWith(suggestions: [], isLoading: false);
      return;
    }

    if (mounted) state = state.copyWith(isLoading: true);

    if (_debounce?.isActive ?? false) _debounce!.cancel();
    _debounce = Timer(const Duration(milliseconds: 300), () {
      _fetchSuggestions(query, service);
    });
  }

  Future<void> _fetchSuggestions(String query, LocationService service) async {
    try {
      final data = await service.findLocationsBy(query);
      if (mounted) {
        state = state.copyWith(suggestions: data, isLoading: false);
      }
    } catch (e) {
      if (mounted) {
        state = state.copyWith(suggestions: [], isLoading: false);
      }
    }
  }

  void setFocus(bool isFocused) {
    if (mounted) {
      state = state.copyWith(isFocused: isFocused);
      if (!isFocused) {
        state = state.copyWith(suggestions: []);
      }
    }
  }

  void clear() {
    if (mounted) {
      state = state.copyWith(suggestions: [], isLoading: false);
    }
  }

  @override
  void dispose() {
    _debounce?.cancel();
    super.dispose();
  }
}