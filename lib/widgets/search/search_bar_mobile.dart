import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/data/icon_service.dart';
import 'package:turbo/data/search/location_service.dart';
import 'package:turbo/l10n/app_localizations.dart';
import 'package:turbo/widgets/map/controller/map_utility.dart';
import 'package:turbo/widgets/search/search_state_provider.dart';

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
  final _textController = TextEditingController();
  final _focusNode = FocusNode();
  final _iconService = IconService();
  OverlayEntry? _overlayEntry;

  @override
  void initState() {
    super.initState();
    _focusNode.addListener(_onFocusChanged);
    _textController.addListener(_onTextChanged);
    Future.microtask(() => ref.read(searchProvider.notifier).clear());
  }

  @override
  void dispose() {
    _focusNode.removeListener(_onFocusChanged);
    _focusNode.dispose();
    _textController.removeListener(_onTextChanged);
    _textController.dispose();
    _removeOverlay();
    super.dispose();
  }

  void _onFocusChanged() {
    debugPrint("[MobileSearch] Focus changed. hasFocus: ${_focusNode.hasFocus}");
    if (_focusNode.hasFocus) {
      _showOverlay();
    } else {
      // This is the key fix. We delay removing the overlay to give the
      // tap event on a suggestion a chance to be processed.
      Future.delayed(const Duration(milliseconds: 200), () {
        // We check if the widgets is still in the tree and if focus
        // hasn't been re-acquired.
        if (mounted && !_focusNode.hasFocus) {
          _removeOverlay();
        }
      });
    }
  }

  void _onTextChanged() {
    // Rebuild to show/hide the clear button
    setState(() {});
    debugPrint("[MobileSearch] Text changed: ${_textController.text}");

    if (_focusNode.hasFocus && _textController.text.isNotEmpty) {
      ref.read(searchProvider.notifier).search(_textController.text);
      _showOverlay();
    } else {
      ref.read(searchProvider.notifier).clear();
      _removeOverlay();
    }
  }

  void _showOverlay() {
    if (_overlayEntry != null) return;
    debugPrint("[MobileSearch] Showing overlay.");

    final overlay = Overlay.of(context);
    final renderBox = context.findRenderObject() as RenderBox;
    final size = renderBox.size;
    final offset = renderBox.localToGlobal(Offset.zero);

    _overlayEntry = OverlayEntry(
      builder: (context) {
        return Positioned(
          top: offset.dy + size.height + 8.0,
          left: 16.0,
          right: 16.0,
          child: _buildSuggestionsList(),
        );
      },
    );
    overlay.insert(_overlayEntry!);
  }

  void _removeOverlay() {
    if (_overlayEntry == null) return;
    debugPrint("[MobileSearch] Removing overlay.");
    _overlayEntry?.remove();
    _overlayEntry = null;
  }

  void _onSuggestionSelected(LocationSearchResult suggestion) {
    debugPrint("[MobileSearch] Tapped on suggestion: ${suggestion.title}");
    _textController.clear();
    // Unfocusing will trigger our _onFocusChanged listener, which will
    // then handle closing the overlay after a delay.
    _unfocusSearchBar();
    ref.read(searchProvider.notifier).clear();
    animatedMapMove(
      suggestion.position,
      13,
      widget.mapController,
      widget.tickerProvider,
    );
  }

  void _unfocusSearchBar() {
    if (_focusNode.hasFocus) {
      _focusNode.unfocus();
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final theme = Theme.of(context);

    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 16.0),
      child: Container(
        height: 56, // Enforce standard M3 SearchBar height
        decoration: BoxDecoration(
          color: theme.colorScheme.surfaceContainer,
          borderRadius: BorderRadius.circular(28.0),
          boxShadow: [
            BoxShadow(
              color: theme.shadowColor.withOpacity(0.1),
              blurRadius: 3,
              offset: const Offset(0, 1),
            ),
          ],
        ),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.center,
          children: [
            const SizedBox(width: 8),
            // Menu Icon Button
            IconButton(
              icon: const Icon(Icons.menu),
              tooltip: l10n.menu,
              onPressed: () {
                _unfocusSearchBar();
                widget.onMenuPressed();
              },
            ),
            const SizedBox(width: 8), // Space between icon and text field

            // Text Field
            Expanded(
              child: TextField(
                controller: _textController,
                focusNode: _focusNode,
                textAlignVertical: TextAlignVertical.top,
                decoration: InputDecoration(
                  hintText: l10n.searchHintMobile,
                  border: InputBorder.none,
                  isDense: true, // Important for vertical alignment
                  contentPadding: EdgeInsets.zero, // Remove all internal padding
                ),
              ),
            ),
            const SizedBox(width: 8), // Space between text field and clear icon

            // Clear Icon Button (conditionally visible)
            if (_textController.text.isNotEmpty)
              IconButton(
                icon: const Icon(Icons.clear),
                onPressed: () {
                  debugPrint("[MobileSearch] Clear button pressed");
                  _textController.clear();
                },
              )
            else
            // This SizedBox keeps the TextField from expanding when the clear
            // button disappears, preventing a layout jump.
              const SizedBox(width: 48), // Default width of an IconButton

            const SizedBox(width: 8),
          ],
        ),
      ),
    );
  }

  Widget _buildSuggestionsList() {
    final l10n = context.l10n;
    return Consumer(
      builder: (context, ref, child) {
        final searchState = ref.watch(searchProvider);
        final theme = Theme.of(context);

        if (_textController.text.trim().length < 2) {
          return const SizedBox.shrink();
        }

        return Material(
          elevation: 3.0,
          color: theme.colorScheme.surfaceContainer,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(28.0),
          ),
          clipBehavior: Clip.antiAlias,
          child: searchState.when(
            loading: () => const Center(
              child: Padding(
                padding: EdgeInsets.symmetric(vertical: 24.0),
                child: CircularProgressIndicator(),
              ),
            ),
            error: (err, stack) => Padding(
              padding: const EdgeInsets.all(16.0),
              child: Text('Error: $err', style: TextStyle(color: theme.colorScheme.error)),
            ),
            data: (suggestions) {
              if (suggestions.isEmpty) {
                return Padding(
                  padding: const EdgeInsets.symmetric(vertical: 24.0),
                  child: Center(child: Text(l10n.noResultsFound)),
                );
              }
              return ConstrainedBox(
                constraints: BoxConstraints(
                  maxHeight: MediaQuery.of(context).size.height * 0.4,
                ),
                child: ListView.builder(
                  padding: const EdgeInsets.symmetric(vertical: 8.0),
                  shrinkWrap: true,
                  itemCount: suggestions.length,
                  itemBuilder: (context, index) {
                    final suggestion = suggestions[index];
                    return ListTile(
                      leading: CircleAvatar(child: _leadingWidget(suggestion)),
                      title: Text(suggestion.title),
                      subtitle: suggestion.description != null &&
                          suggestion.description!.isNotEmpty
                          ? Text(suggestion.description!,
                          maxLines: 1, overflow: TextOverflow.ellipsis)
                          : null,
                      onTap: () {
                        debugPrint(
                            "[MobileSearch] ListTile tapped for ${suggestion.title}");
                        _onSuggestionSelected(suggestion);
                      },
                    );
                  },
                ),
              );
            },
          ),
        );
      },
    );
  }

  Widget _leadingWidget(LocationSearchResult suggestion) {
    if (suggestion.icon != null) {
      return Icon(_iconService.getIcon(context, suggestion.icon!).icon);
    }
    return Text(suggestion.title.isNotEmpty ? suggestion.title[0] : '?');
  }
}