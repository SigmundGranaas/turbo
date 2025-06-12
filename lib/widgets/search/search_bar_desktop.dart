import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:map_app/data/icon_service.dart';
import 'package:map_app/data/search/composite_search_service.dart';
import 'package:map_app/data/search/location_service.dart';
import 'package:map_app/widgets/map/controller/map_utility.dart';

class DesktopSearchBar extends ConsumerStatefulWidget {
  final MapController mapController;
  final TickerProvider tickerProvider;

  const DesktopSearchBar({
    super.key,
    required this.mapController,
    required this.tickerProvider,
  });

  @override
  ConsumerState<DesktopSearchBar> createState() => _DesktopSearchBarState();
}

class _DesktopSearchBarState extends ConsumerState<DesktopSearchBar> {
  final TextEditingController _controller = TextEditingController();
  final FocusNode _focusNode = FocusNode();
  final LayerLink _layerLink = LayerLink();
  final IconService _iconService = IconService();

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
        _updateOverlay();
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
    final renderBox = context.findRenderObject() as RenderBox;
    final size = renderBox.size;

    return OverlayEntry(
      builder: (context) => Positioned(
        width: size.width,
        child: CompositedTransformFollower(
          link: _layerLink,
          showWhenUnlinked: false,
          offset: Offset(0.0, size.height + 8.0),
          child: Material(
            elevation: 4,
            shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(16)),
            clipBehavior: Clip.antiAlias,
            child: ConstrainedBox(
              constraints: BoxConstraints(maxHeight: MediaQuery.of(context).size.height * 0.5),
              child: ListView.builder(
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
        ),
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

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;

    return CompositedTransformTarget(
      link: _layerLink,
      child: SizedBox(
        width: 450,
        height: 64,
        child: Card(
          elevation: 4,
          shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(32)),
          clipBehavior: Clip.antiAlias,
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.center,
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
                    isCollapsed: true,
                  ),
                  onChanged: _onSearchChanged,
                ),
              ),
              if (_isLoading)
                const Padding(
                  padding: EdgeInsets.all(16.0),
                  child: SizedBox(width: 24, height: 24, child: CircularProgressIndicator(strokeWidth: 2.5)),
                )
              else if (_controller.text.isNotEmpty)
                IconButton(
                  icon: Icon(Icons.close, color: colorScheme.onSurfaceVariant),
                  onPressed: () {
                    _controller.clear();
                    _onSearchChanged('');
                  },
                  tooltip: 'Clear search',
                ),
              if (_controller.text.isEmpty && !_isLoading) const SizedBox(width: 12),
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