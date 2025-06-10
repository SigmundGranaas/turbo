import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:map_app/data/icon_service.dart';
import 'package:map_app/data/search/composite_search_service.dart';
import 'package:map_app/data/search/location_service.dart';
import 'package:map_app/widgets/search/search_logic.dart';

class DesktopSearchBar extends ConsumerStatefulWidget {
  final Function(double, double) onLocationSelected;

  const DesktopSearchBar({
    super.key,
    required this.onLocationSelected,
  });

  @override
  ConsumerState<DesktopSearchBar> createState() => _DesktopSearchBarState();
}

class _DesktopSearchBarState extends ConsumerState<DesktopSearchBar> {
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
    widget.onLocationSelected(suggestion.position.longitude, suggestion.position.latitude);
    _controller.clear();
    _focusNode.unfocus();
    ref.read(searchLogicProvider.notifier).clear();
  }

  @override
  Widget build(BuildContext context) {
    final searchState = ref.watch(searchLogicProvider);
    final searchNotifier = ref.read(searchLogicProvider.notifier);
    final searchService = ref.watch(compositeSearchServiceProvider);
    final colorScheme = Theme.of(context).colorScheme;

    return SizedBox(
      width: 450,
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          SizedBox(
            height: 64, // Match burger menu button height
            child: Card(
              elevation: 4,
              shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(32)), // Make it pill-shaped
              clipBehavior: Clip.antiAlias,
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.center, // Explicitly center children vertically
                children: [
                  const SizedBox(width: 20),
                  Icon(Icons.search, color: colorScheme.onSurfaceVariant),
                  const SizedBox(width: 12),
                  Expanded(
                    child: TextField(
                      controller: _controller,
                      focusNode: _focusNode,
                      decoration: const InputDecoration(
                        hintText: 'Search places, coordinates...',
                        border: InputBorder.none,
                        isCollapsed: true, // This is crucial for vertical centering
                      ),
                      onChanged: (query) => searchNotifier.onSearchChanged(query, searchService),
                    ),
                  ),
                  if (searchState.isLoading)
                    const Padding(
                      padding: EdgeInsets.all(16.0),
                      child: SizedBox(width: 24, height: 24, child: CircularProgressIndicator(strokeWidth: 2.5)),
                    )
                  else if (_controller.text.isNotEmpty)
                    IconButton(
                      icon: Icon(Icons.close, color: colorScheme.onSurfaceVariant),
                      onPressed: () {
                        _controller.clear();
                        searchNotifier.clear();
                      },
                      tooltip: 'Clear search',
                    ),
                  if (_controller.text.isEmpty && !searchState.isLoading) const SizedBox(width: 12),
                ],
              ),
            ),
          ),
          if (searchState.isFocused && searchState.suggestions.isNotEmpty)
            Flexible(
              child: Card(
                margin: const EdgeInsets.only(top: 8),
                elevation: 4,
                shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(16)),
                child: Container(
                  constraints: BoxConstraints(
                    maxHeight: MediaQuery.of(context).size.height * 0.5,
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