import 'dart:math';
import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/app/tokens.dart';
import 'package:turbo/core/widgets/app_button.dart';
import 'package:turbo/core/widgets/app_grouped_card.dart';
import 'package:turbo/core/widgets/app_text_field.dart';
import 'package:turbo/features/tile_providers/api.dart';
import 'package:turbo/features/tile_storage/offline_regions/api.dart'
as offline_api;
import 'package:turbo/app/l10n/app_localizations.dart';

class DownloadDetailsSheet extends ConsumerStatefulWidget {
  final LatLngBounds bounds;
  const DownloadDetailsSheet({super.key, required this.bounds});

  @override
  ConsumerState<DownloadDetailsSheet> createState() =>
      _DownloadDetailsSheetState();
}

class _DownloadDetailsSheetState extends ConsumerState<DownloadDetailsSheet> {
  final _formKey = GlobalKey<FormState>();
  final _nameController = TextEditingController();
  RangeValues _zoomRange = const RangeValues(10, 14);
  String? _selectedProviderId;
  int _tileCount = 0;
  int _estimatedSizeMb = 0;
  double _providerMinZoom = 1;
  double _providerMaxZoom = 19;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) {
        _nameController.text = AppLocalizations.of(context).defaultOfflineMapName;
      }
      _initializeProvider();
    });
  }

  void _initializeProvider() {
    if (!mounted) return;
    final registry = ref.read(tileRegistryProvider);
    final activeLocalId = registry.activeLocalIds.firstOrNull;
    final activeGlobalId = registry.activeGlobalIds.firstOrNull;

    final downloadableProviders = registry.availableProviders.values
        .where((p) =>
    p.category == TileProviderCategory.global ||
        p.category == TileProviderCategory.local)
        .toList();

    if (downloadableProviders.isEmpty) return;

    final defaultProviderId = activeLocalId ?? activeGlobalId;

    final providerIdToSet =
    downloadableProviders.any((p) => p.id == defaultProviderId)
        ? defaultProviderId
        : downloadableProviders.first.id;

    _setSelectedProvider(providerIdToSet);
  }

  void _setSelectedProvider(String? providerId) {
    if (providerId == null) return;
    final provider =
    ref.read(tileRegistryProvider).availableProviders[providerId];
    if (provider == null) return;

    setState(() {
      _selectedProviderId = providerId;
      _providerMinZoom = provider.minZoom;
      _providerMaxZoom = provider.maxZoom;
      // Clamp the current zoom range to the new provider's limits
      _zoomRange = RangeValues(
        _zoomRange.start.clamp(_providerMinZoom, _providerMaxZoom),
        _zoomRange.end.clamp(_providerMinZoom, _providerMaxZoom),
      );
    });
    _updateEstimates();
  }

  void _updateEstimates() {
    if (!mounted) return;
    const crs = Epsg3857();
    const tileSize = 256.0;
    int count = 0;
    for (var z = _zoomRange.start.round(); z <= _zoomRange.end.round(); z++) {
      final zoom = z.toDouble();
      final nwPoint = crs.latLngToOffset(widget.bounds.northWest, zoom);
      final sePoint = crs.latLngToOffset(widget.bounds.southEast, zoom);
      final from = Point<int>(
          (nwPoint.dx / tileSize).floor(), (nwPoint.dy / tileSize).floor());
      final to = Point<int>(
          (sePoint.dx / tileSize).floor(), (sePoint.dy / tileSize).floor());
      count += (to.x - from.x + 1) * (to.y - from.y + 1);
    }
    setState(() {
      _tileCount = count;
      _estimatedSizeMb = (count * 25 / 1024).round(); // Avg 25KB per tile
    });
  }

  void _startDownload() async {
    if (!(_formKey.currentState?.validate() ?? false) ||
        _selectedProviderId == null) {
      return;
    }

    final provider =
    ref.read(tileRegistryProvider).availableProviders[_selectedProviderId!];
    if (provider == null) return;

    final scheduled =
        await ref.read(offline_api.offlineRegionsProvider.notifier).createRegion(
              name: _nameController.text.trim(),
              bounds: widget.bounds,
              minZoom: _zoomRange.start.round(),
              maxZoom: _zoomRange.end.round(),
              urlTemplate: provider.urlTemplate,
              tileProviderId: provider.id,
              tileProviderName: provider.name(context),
            );

    if (!mounted) return;
    if (!scheduled) {
      // The platform refused the download (web has no local tile store) —
      // tell the user instead of silently popping back to the root.
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(context.l10n.offlineMapsNotAvailableOnWeb)),
      );
      return;
    }
    Navigator.of(context).popUntil((route) => route.isFirst);
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final tileRegistry = ref.watch(tileRegistryProvider);
    final downloadableProviders = tileRegistry.availableProviders.values
        .where((p) =>
            p.category == TileProviderCategory.global ||
            p.category == TileProviderCategory.local)
        .toList();

    return SafeArea(
      child: SingleChildScrollView(
        child: Padding(
          padding: EdgeInsets.fromLTRB(
              AppSpacing.l,
              AppSpacing.l,
              AppSpacing.l,
              AppSpacing.l + MediaQuery.of(context).viewInsets.bottom),
          child: Form(
            key: _formKey,
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Text(l10n.downloadDetails,
                    style: Theme.of(context).textTheme.titleLarge),
                const SizedBox(height: AppSpacing.xl),
                AppTextField(
                  controller: _nameController,
                  label: l10n.regionName,
                  validator: (v) =>
                      v!.trim().isEmpty ? l10n.pleaseEnterName : null,
                ),
                const SizedBox(height: AppSpacing.l),
                if (downloadableProviders.isNotEmpty)
                  Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(l10n.mapSource,
                          style: Theme.of(context).textTheme.labelMedium),
                      const SizedBox(height: AppSpacing.s),
                      MenuAnchor(
                        builder: (context, controller, child) {
                          final selectedProvider =
                              downloadableProviders.firstWhere(
                            (p) => p.id == _selectedProviderId,
                            orElse: () => downloadableProviders.first,
                          );
                          return OutlinedButton(
                            onPressed: () {
                              if (controller.isOpen) {
                                controller.close();
                              } else {
                                controller.open();
                              }
                            },
                            style: OutlinedButton.styleFrom(
                              minimumSize: const Size.fromHeight(56),
                              shape: RoundedRectangleBorder(
                                  borderRadius:
                                      BorderRadius.circular(AppRadius.l)),
                              side: BorderSide(
                                  color: Theme.of(context).colorScheme.outline),
                            ),
                            child: Row(
                              mainAxisAlignment:
                                  MainAxisAlignment.spaceBetween,
                              children: [
                                Text(selectedProvider.name(context)),
                                const Icon(Icons.arrow_drop_down),
                              ],
                            ),
                          );
                        },
                        menuChildren: downloadableProviders.map((p) {
                          return MenuItemButton(
                            onPressed: () => _setSelectedProvider(p.id),
                            child: Padding(
                              padding: const EdgeInsets.symmetric(
                                  horizontal: AppSpacing.l),
                              child: Text(p.name(context)),
                            ),
                          );
                        }).toList(),
                      ),
                    ],
                  )
                else
                  Text(l10n.noDownloadableMapSources),
                const SizedBox(height: AppSpacing.xl),
                Text(
                    "Zoom Range: ${_zoomRange.start.round()} - ${_zoomRange.end.round()}",
                    style: Theme.of(context).textTheme.titleSmall),
                RangeSlider(
                  values: _zoomRange,
                  min: _providerMinZoom,
                  max: _providerMaxZoom,
                  divisions: (_providerMaxZoom - _providerMinZoom).round(),
                  labels: RangeLabels(_zoomRange.start.round().toString(),
                      _zoomRange.end.round().toString()),
                  onChanged: (v) {
                    setState(() => _zoomRange = v);
                    _updateEstimates();
                  },
                ),
                const SizedBox(height: AppSpacing.l),
                AppGroupedCard(
                  padding: const EdgeInsets.all(AppSpacing.m),
                  child: Text(
                    '${l10n.estimatedTiles(_tileCount)}\n${l10n.estimatedSizeMb(_estimatedSizeMb)}',
                    textAlign: TextAlign.center,
                    style: Theme.of(context).textTheme.bodyMedium,
                  ),
                ),
                const SizedBox(height: AppSpacing.xl),
                AppButton.primary(
                  text: l10n.startDownload,
                  icon: Icons.download,
                  onPressed: _startDownload,
                  fullWidth: true,
                )
              ],
            ),
          ),
        ),
      ),
    );
  }
}