import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:turbo/core/overlay/transient_surface.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:logging/logging.dart';
import 'package:turbo/core/connectivity/connectivity_provider.dart';
import 'package:turbo/features/markers/api.dart';
import 'package:turbo/app/l10n/app_localizations.dart';
import 'package:turbo/core/widgets/map/controller/map_utility.dart';
import '../data/location_service.dart';
import '../data/search_state_provider.dart';

final _log = Logger('DesktopSearch');

class DesktopSearchBar extends ConsumerStatefulWidget {
  final MapController mapController;
  final TickerProvider tickerProvider;

  /// Called after a result is picked (and the map has panned to it). The shell
  /// selects the result so the shared detail/action bar appears. Keeps search
  /// decoupled from the selection seam.
  final void Function(LocationSearchResult result)? onResultSelected;

  const DesktopSearchBar({
    super.key,
    required this.mapController,
    required this.tickerProvider,
    this.onResultSelected,
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
    _log.fine(() => 'Focus changed. hasFocus: ${_focusNode.hasFocus}');
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
    _log.fine(() => 'Text changed: ${_textController.text}');

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
    _log.fine('Showing overlay');

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
    TransientSurface.setSearchDismisser(_removeOverlay);
  }

  void _removeOverlay() {
    if (_overlayEntry == null) return;
    _log.fine('Removing overlay');
    _overlayEntry?.remove();
    _overlayEntry = null;
    TransientSurface.clearSearchDismisser(_removeOverlay);
  }

  void _onSuggestionSelected(LocationSearchResult suggestion) {
    _log.fine(() => 'Tapped on suggestion: ${suggestion.title}');
    _textController.clear();
    // Unfocusing will trigger our _onFocusChanged listener, which will
    // then handle closing the overlay after a delay.
    _focusNode.unfocus();
    ref.read(searchProvider.notifier).clear();
    animatedMapMove(
      suggestion.position,
      13,
      widget.mapController,
      widget.tickerProvider,
    );
    widget.onResultSelected?.call(suggestion);
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final theme = Theme.of(context);

    return CompositedTransformTarget(
      link: _layerLink,
      child: SizedBox(
        width: 450,
        height: 56, // Enforce standard M3 SearchBar height
        child: Container(
          decoration: BoxDecoration(
            color: theme.colorScheme.surfaceContainer,
            borderRadius: BorderRadius.circular(28.0),
            boxShadow: [
              BoxShadow(
                color: theme.shadowColor.withValues(alpha: 0.1),
                blurRadius: 3,
                offset: const Offset(0, 1),
              ),
            ],
          ),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              Padding(
                padding: const EdgeInsets.symmetric(horizontal: 16.0),
                child: Icon(Icons.search, color: theme.colorScheme.onSurface),
              ),
              Expanded(
                child: TextField(
                  controller: _textController,
                  focusNode: _focusNode,
                  textAlignVertical: TextAlignVertical.top,
                  decoration: InputDecoration(
                    hintText: l10n.searchHint,
                    isDense: true,
                    border: InputBorder.none,
                    enabledBorder: InputBorder.none,
                    focusedBorder: InputBorder.none,
                    disabledBorder: InputBorder.none,
                    errorBorder: InputBorder.none,
                    focusedErrorBorder: InputBorder.none,
                    contentPadding: EdgeInsets.zero,
                  ),
                ),
              ),
              if (_textController.text.isNotEmpty)
                Padding(
                  padding: const EdgeInsets.only(right: 8.0),
                  child: IconButton(
                    icon: const Icon(Icons.clear),
                    onPressed: () {
                      _log.fine('Clear button pressed');
                      _textController.clear();
                    },
                  ),
                ),
              const SizedBox(width: 8),
            ],
          ),
        ),
      ),
    );
  }

  Widget _buildSuggestionsList() {
    final l10n = context.l10n;
    return Consumer(
      builder: (context, ref, child) {
        final searchState = ref.watch(searchProvider);
        final isOnline = ref.watch(connectivityProvider);
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
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              if (!isOnline) _OfflineHintBanner(l10n: l10n),
              Flexible(child: searchState.when(
            loading: () => const Center(
              child: Padding(
                padding: EdgeInsets.symmetric(vertical: 24.0),
                child: CircularProgressIndicator(),
              ),
            ),
            error: (err, stack) => Padding(
              padding: const EdgeInsets.all(16.0),
              child: Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: [
                  Expanded(
                    child: Text(
                      l10n.searchFailed,
                      style: theme.textTheme.bodyMedium
                          ?.copyWith(color: theme.colorScheme.error),
                    ),
                  ),
                  TextButton(
                    onPressed: () => ref
                        .read(searchProvider.notifier)
                        .search(_textController.text),
                    child: Text(l10n.retry),
                  ),
                ],
              ),
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
                      subtitle: suggestion.description != null &&
                          suggestion.description!.isNotEmpty
                          ? Text(suggestion.description!,
                          maxLines: 1, overflow: TextOverflow.ellipsis)
                          : null,
                      onTap: () {
                        _log.fine(() =>
                            'ListTile tapped for ${suggestion.title}');
                        _onSuggestionSelected(suggestion);
                      },
                    );
                  },
                ),
              );
            },
          )),
            ],
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

class _OfflineHintBanner extends StatelessWidget {
  final AppLocalizations l10n;
  const _OfflineHintBanner({required this.l10n});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Container(
      width: double.infinity,
      color: theme.colorScheme.tertiaryContainer,
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
      child: Row(
        children: [
          Icon(
            Icons.cloud_off_outlined,
            size: 16,
            color: theme.colorScheme.onTertiaryContainer,
          ),
          const SizedBox(width: 8),
          Expanded(
            child: Text(
              l10n.searchOfflineHint,
              style: theme.textTheme.bodySmall?.copyWith(
                color: theme.colorScheme.onTertiaryContainer,
              ),
            ),
          ),
        ],
      ),
    );
  }
}