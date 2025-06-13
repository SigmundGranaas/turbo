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
  final TextEditingController _controller = TextEditingController();
  final FocusNode _focusNode = FocusNode();
  final LayerLink _layerLink = LayerLink();
  final IconService _iconService = IconService();
  final GlobalKey _searchBarKey = GlobalKey(); // Add this key

  OverlayEntry? _overlayEntry;
  List<LocationSearchResult> _suggestions = [];
  bool _isLoading = false;
  Timer? _debounce;

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
    _debounce?.cancel();
    _removeOverlay();
    super.dispose();
  }

  void _onFocusChange() {
    if (_focusNode.hasFocus) {
      _showOverlay();
    } else {
      // Delay removal to allow tap events to be processed.
      Future.delayed(const Duration(milliseconds: 200), () {
        if (mounted && !_focusNode.hasFocus) {
          _removeOverlay();
        }
      });
    }
  }

  void _onSearchChanged(String query) {
    final searchService = ref.read(compositeSearchServiceProvider);
    if (_debounce?.isActive ?? false) _debounce!.cancel();

    if (query.length < 2) {
      setState(() {
        _isLoading = false;
        _suggestions = [];
      });
      _updateOverlay();
      return;
    }

    setState(() => _isLoading = true);
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
      } finally {
        if (mounted) {
          _updateOverlay();
        }
      }
    });
  }

  void _onSuggestionSelected(LocationSearchResult suggestion) {
    // Unfocus the text field. This triggers the delayed removal in _onFocusChange.
    _focusNode.unfocus();
    _controller.clear();
    setState(() {
      _suggestions = [];
    });

    // Animate the map.
    animatedMapMove(
      suggestion.position,
      13,
      widget.mapController,
      widget.tickerProvider,
    );
  }

  void _showOverlay() {
    if (_overlayEntry != null) return;
    _overlayEntry = _createOverlayEntry();
    Overlay.of(context).insert(_overlayEntry!);
  }

  void _removeOverlay() {
    _overlayEntry?.remove();
    _overlayEntry = null;
  }

  void _updateOverlay() {
    _overlayEntry?.markNeedsBuild();
  }

  OverlayEntry _createOverlayEntry() {
    return OverlayEntry(
      builder: (context) {
        // Get the search bar's size and position from the key
        final RenderBox? renderBox = _searchBarKey.currentContext?.findRenderObject() as RenderBox?;
        if (renderBox == null) {
          return const SizedBox.shrink();
        }

        final size = renderBox.size;
        final position = renderBox.localToGlobal(Offset.zero);
        final screenWidth = MediaQuery.of(context).size.width;

        // Calculate the overlay width with proper constraints
        final overlayWidth = size.width;
        final leftOffset = position.dx;

        // Ensure the overlay doesn't go off screen
        final maxWidth = screenWidth - leftOffset - 16; // 16 for padding
        final finalWidth = overlayWidth.clamp(0.0, maxWidth);

        return Positioned(
          left: leftOffset,
          top: position.dy + size.height + 8.0,
          child: Material(
            elevation: 4,
            shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(16)),
            clipBehavior: Clip.antiAlias,
            child: Container(
              width: finalWidth,
              constraints: BoxConstraints(
                maxHeight: MediaQuery.of(context).size.height * 0.4,
                maxWidth: finalWidth,
              ),
              child: _suggestions.isEmpty && !_isLoading
                  ? const SizedBox.shrink()
                  : _isLoading
                  ? Container(
                padding: const EdgeInsets.all(16),
                child: const Center(
                  child: CircularProgressIndicator(),
                ),
              )
                  : ListView.builder(
                padding: EdgeInsets.zero,
                shrinkWrap: true,
                itemCount: _suggestions.length,
                itemBuilder: (context, index) {
                  final suggestion = _suggestions[index];
                  return _buildSuggestionItem(suggestion);
                },
              ),
            ),
          ),
        );
      },
    );
  }

  Widget _buildSuggestionItem(LocationSearchResult suggestion) {
    return ListTile(
      leading: CircleAvatar(
        backgroundColor: Theme.of(context).colorScheme.primaryContainer,
        child: _leadingWidget(suggestion),
      ),
      title: Text(
        suggestion.title,
        overflow: TextOverflow.ellipsis,
      ),
      subtitle: suggestion.description != null
          ? Text(
        suggestion.description!,
        overflow: TextOverflow.ellipsis,
      )
          : null,
      onTap: () => _onSuggestionSelected(suggestion),
    );
  }

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;

    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 16.0),
      child: CompositedTransformTarget(
        link: _layerLink,
        child: Card(
          key: _searchBarKey, // Add the key here
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
                  onChanged: _onSearchChanged,
                ),
              ),
              if (_focusNode.hasFocus)
                IconButton(
                  icon: Icon(Icons.close, color: colorScheme.onSurfaceVariant),
                  onPressed: () {
                    _controller.clear();
                    _onSearchChanged('');
                    _focusNode.unfocus();
                  },
                  tooltip: 'Clear search',
                ),
              const SizedBox(width: 4),
            ],
          ),
        ),
      ),
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