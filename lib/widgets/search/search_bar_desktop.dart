import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/data/icon_service.dart';
import 'package:turbo/data/search/location_service.dart';
import 'package:turbo/l10n/app_localizations.dart';
import 'package:turbo/widgets/map/controller/map_utility.dart';
import 'package:turbo/widgets/search/search_state_provider.dart';

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
  final _textController = TextEditingController();
  final _focusNode = FocusNode();
  final _iconService = IconService();
  final _layerLink = LayerLink();
  OverlayEntry? _overlayEntry;

  @override
  void initState() {
    super.initState();
    _focusNode.addListener(_onFocusChanged);
    _textController.addListener(_onTextChanged);
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
    if (_focusNode.hasFocus && _textController.text.isNotEmpty) {
      _showOverlay();
    } else {
      _removeOverlay();
    }
  }

  void _onTextChanged() {
    if (_textController.text.isNotEmpty && _focusNode.hasFocus) {
      _showOverlay();
    } else {
      _removeOverlay();
    }
    ref.read(searchProvider.notifier).search(_textController.text);
  }

  void _showOverlay() {
    if (_overlayEntry != null) return;

    final overlay = Overlay.of(context);
    final renderBox = context.findRenderObject() as RenderBox;
    final size = renderBox.size;

    _overlayEntry = OverlayEntry(
      builder: (context) => Positioned(
        width: size.width,
        child: CompositedTransformFollower(
          link: _layerLink,
          showWhenUnlinked: false,
          offset: Offset(0, size.height + 8.0),
          child: _buildSuggestionsList(),
        ),
      ),
    );
    overlay.insert(_overlayEntry!);
  }

  void _removeOverlay() {
    _overlayEntry?.remove();
    _overlayEntry = null;
  }

  void _onSuggestionSelected(LocationSearchResult suggestion) {
    _textController.clear();
    _focusNode.unfocus();
    ref.read(searchProvider.notifier).clear();
    animatedMapMove(
      suggestion.position,
      13,
      widget.mapController,
      widget.tickerProvider,
    );
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    return CompositedTransformTarget(
      link: _layerLink,
      child: SizedBox(
        width: 450,
        child: SearchBar(
          controller: _textController,
          focusNode: _focusNode,
          hintText: l10n.searchHint,
          leading: const Icon(Icons.search),
          padding: const WidgetStatePropertyAll<EdgeInsets>(
              EdgeInsets.symmetric(horizontal: 16.0)),
          trailing: [
            if (_textController.text.isNotEmpty)
              IconButton(
                icon: const Icon(Icons.clear),
                onPressed: () {
                  _textController.clear();
                  ref.read(searchProvider.notifier).clear();
                },
              ),
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
                constraints: const BoxConstraints(maxHeight: 350),
                child: ListView.builder(
                  padding: const EdgeInsets.symmetric(vertical: 8.0),
                  shrinkWrap: true,
                  itemCount: suggestions.length,
                  itemBuilder: (context, index) {
                    final suggestion = suggestions[index];
                    return ListTile(
                      leading: CircleAvatar(child: _leadingWidget(suggestion)),
                      title: Text(suggestion.title),
                      subtitle: suggestion.description != null && suggestion.description!.isNotEmpty
                          ? Text(suggestion.description!, maxLines: 1, overflow: TextOverflow.ellipsis)
                          : null,
                      onTap: () => _onSuggestionSelected(suggestion),
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