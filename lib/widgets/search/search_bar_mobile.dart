import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:map_app/data/icon_service.dart';
import 'package:map_app/data/search/composite_search_service.dart';
import 'package:map_app/data/search/location_service.dart';
import 'package:map_app/widgets/map/controller/map_utility.dart';
import 'package:map_app/widgets/search/search_logic.dart';

class MobileSearchBar extends ConsumerStatefulWidget {
  final MapController mapController;
  final TickerProvider tickerProvider;
  final VoidCallback onMenuPressed;

  const MobileSearchBar({
    super.key,
    required this.mapController,
    required this.tickerProvider,
    required this.onMenuPressed,
  });

  @override
  ConsumerState<MobileSearchBar> createState() => _MobileSearchBarState();
}

class _MobileSearchBarState extends ConsumerState<MobileSearchBar> {
  final TextEditingController _controller = TextEditingController();
  final FocusNode _focusNode = FocusNode();
  final IconService _iconService = IconService();

  @override
  void initState() {
    super.initState();
    _focusNode.addListener(_onFocusChange);
  }

  @override
  void dispose() {
    _focusNode.removeListener(_onFocusChange);
    _focusNode.dispose();
    _controller.dispose();
    super.dispose();
  }

  void _onFocusChange() {
    ref.read(searchLogicProvider.notifier).setFocus(_focusNode.hasFocus);
  }

  void _onSuggestionSelected(LocationSearchResult suggestion) {
    // 1. Immediately fire the animation. It's a "fire-and-forget" call.
    animatedMapMove(suggestion.position, 13, widget.mapController, widget.tickerProvider);

    // 2. Schedule the UI cleanup to run AFTER the current frame is built.
    // This is the definitive fix for the gesture cancellation.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) {
        _focusNode.unfocus();
        _controller.clear();
        ref.read(searchLogicProvider.notifier).clear();
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    final searchState = ref.watch(searchLogicProvider);
    final searchNotifier = ref.read(searchLogicProvider.notifier);
    final searchService = ref.watch(compositeSearchServiceProvider);
    final colorScheme = Theme.of(context).colorScheme;

    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 16.0),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Card(
            elevation: 4,
            shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(28)),
            clipBehavior: Clip.antiAlias,
            child: Row(
              children: [
                const SizedBox(width: 4),
                IconButton(
                  icon: Icon(Icons.menu, color: colorScheme.onSurfaceVariant),
                  onPressed: widget.onMenuPressed,
                  tooltip: 'Open menu',
                ),
                Expanded(
                  child: TextField(
                    controller: _controller,
                    focusNode: _focusNode,
                    decoration: const InputDecoration(
                      hintText: 'Search here',
                      border: InputBorder.none,
                    ),
                    onChanged: (query) => searchNotifier.onSearchChanged(query, searchService),
                  ),
                ),
                if (_focusNode.hasFocus)
                  IconButton(
                    icon: Icon(Icons.close, color: colorScheme.onSurfaceVariant),
                    onPressed: () {
                      _controller.clear();
                      searchNotifier.clear();
                      _focusNode.unfocus();
                    },
                    tooltip: 'Clear search',
                  ),
                const SizedBox(width: 4),
              ],
            ),
          ),
          if (searchState.isFocused && searchState.suggestions.isNotEmpty)
            Flexible(
              child: Card(
                margin: const EdgeInsets.only(top: 8),
                elevation: 4,
                shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(16)),
                clipBehavior: Clip.antiAlias, // This fixes the highlight overflow
                child: Container(
                  constraints: BoxConstraints(
                    maxHeight: MediaQuery.of(context).size.height * 0.4,
                  ),
                  child: ListView.builder(
                    padding: EdgeInsets.zero,
                    shrinkWrap: true,
                    itemCount: searchState.suggestions.length,
                    itemBuilder: (context, index) {
                      final suggestion = searchState.suggestions[index];
                      return _buildSuggestionItem(suggestion);
                    },
                  ),
                ),
              ),
            ),
        ],
      ),
    );
  }

  Widget _buildSuggestionItem(LocationSearchResult suggestion) {
    return ListTile(
      leading: CircleAvatar(
        backgroundColor: Theme.of(context).colorScheme.primaryContainer,
        child: _leadingWidget(suggestion),
      ),
      title: Text(suggestion.title),
      subtitle: Text(suggestion.description ?? ''),
      onTap: () => _onSuggestionSelected(suggestion),
    );
  }

  Widget _leadingWidget(LocationSearchResult suggestion) {
    if (suggestion.icon != null) {
      return Icon(
        _iconService.getIcon(suggestion.icon).icon,
        color: Theme.of(context).colorScheme.onPrimaryContainer,
      );
    } else {
      return Text(
        suggestion.title.isNotEmpty ? suggestion.title[0].toUpperCase() : '?',
        style: TextStyle(
          color: Theme.of(context).colorScheme.onPrimaryContainer,
          fontWeight: FontWeight.bold,
        ),
      );
    }
  }
}