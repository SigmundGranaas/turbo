import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/data/icon_service.dart';
import 'package:turbo/data/search/composite_search_service.dart';
import 'package:turbo/data/search/location_service.dart';
import 'package:turbo/widgets/map/controller/map_utility.dart';

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
  final SearchController _controller = SearchController();
  final IconService _iconService = IconService();

  Timer? _debounce;
  List<LocationSearchResult> _suggestions = [];
  bool _isLoading = false;

  @override
  void initState() {
    super.initState();
    _controller.addListener(_onSearchChanged);
  }

  @override
  void dispose() {
    _debounce?.cancel();
    _controller.removeListener(_onSearchChanged);
    _controller.dispose();
    super.dispose();
  }

  void _onSearchChanged() {
    final String query = _controller.text;
    final searchService = ref.read(compositeSearchServiceProvider);
    if (_debounce?.isActive ?? false) _debounce!.cancel();

    if (query.trim().length < 2) {
      if (mounted) {
        setState(() {
          _isLoading = false;
          _suggestions = [];
        });
      }
      return;
    }

    if (mounted) setState(() => _isLoading = true);
    _debounce = Timer(const Duration(milliseconds: 300), () async {
      try {
        final data = await searchService.findLocationsBy(query);
        if (mounted) {
          setState(() {
            _suggestions = data;
            _isLoading = false;
          });
        }
      } catch (e) {
        if (mounted) {
          setState(() {
            _suggestions = [];
            _isLoading = false;
          });
        }
      }
    });
  }

  void _onSuggestionSelected(LocationSearchResult suggestion) {
    _controller.closeView(suggestion.title);
    FocusScope.of(context).unfocus();
    animatedMapMove(
      suggestion.position,
      13,
      widget.mapController,
      widget.tickerProvider,
    );
  }

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 16.0),
      child: SearchAnchor(
        searchController: _controller,
        builder: (BuildContext context, SearchController controller) {
          return SearchBar(
            controller: controller,
            padding: const WidgetStatePropertyAll<EdgeInsets>(
                EdgeInsets.only(left: 8.0, right: 16.0)),
            onTap: () => controller.openView(),
            onChanged: (_) => controller.openView(),
            leading: IconButton(
              icon: const Icon(Icons.menu),
              onPressed: widget.onMenuPressed,
            ),
            hintText: "Search places...",
            trailing: [
              if (_isLoading)
                const SizedBox(
                  width: 20,
                  height: 20,
                  child: CircularProgressIndicator(strokeWidth: 2.5),
                )
            ],
          );
        },
        suggestionsBuilder:
            (BuildContext context, SearchController controller) {
          if (_isLoading) {
            return [
              const Center(
                  child: Padding(
                      padding: EdgeInsets.all(16.0),
                      child: CircularProgressIndicator()))
            ];
          }
          if (_suggestions.isEmpty && controller.text.isNotEmpty) {
            return [
              const Center(
                  child: Padding(
                      padding: EdgeInsets.all(16.0),
                      child: Text('No results found.')))
            ];
          }
          return _suggestions.map((s) => _buildSuggestionItem(s));
        },
      ),
    );
  }

  Widget _buildSuggestionItem(LocationSearchResult suggestion) {
    return ListTile(
      leading: CircleAvatar(
        child: _leadingWidget(suggestion),
      ),
      title: Text(suggestion.title),
      subtitle: Text(suggestion.description ?? ''),
      onTap: () => _onSuggestionSelected(suggestion),
    );
  }

  Widget _leadingWidget(LocationSearchResult suggestion) {
    if (suggestion.icon != null) {
      return Icon(_iconService.getIcon(suggestion.icon).icon);
    } else {
      return Text(suggestion.title.isNotEmpty ? suggestion.title[0] : '?');
    }
  }
}