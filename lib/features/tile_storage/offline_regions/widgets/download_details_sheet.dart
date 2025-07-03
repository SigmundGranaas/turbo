import 'dart:math';
import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:turbo/features/tile_providers/api.dart';
import 'package:turbo/features/tile_storage/offline_regions/api.dart'
as offline_api;

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
    _nameController.text = "My Offline Map";
    WidgetsBinding.instance.addPostFrameCallback((_) => _initializeProvider());
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
      final nwPoint = crs.latLngToPoint(widget.bounds.northWest, zoom);
      final sePoint = crs.latLngToPoint(widget.bounds.southEast, zoom);
      final from = Point<int>(
          (nwPoint.x / tileSize).floor(), (nwPoint.y / tileSize).floor());
      final to = Point<int>(
          (sePoint.x / tileSize).floor(), (sePoint.y / tileSize).floor());
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

    final offlineApi = ref.read(offline_api.offlineApiProvider);
    // No need to await, the process runs in the background.
    offlineApi.downloadRegion(
      name: _nameController.text.trim(),
      bounds: widget.bounds,
      minZoom: _zoomRange.start.round(),
      maxZoom: _zoomRange.end.round(),
      urlTemplate: provider.urlTemplate,
      tileProviderId: provider.id,
      tileProviderName: provider.name(context),
    );

    if (!mounted) return;
    Navigator.of(context).popUntil((route) => route.isFirst);
  }

  @override
  Widget build(BuildContext context) {
    final tileRegistry = ref.watch(tileRegistryProvider);
    final downloadableProviders = tileRegistry.availableProviders.values
        .where((p) =>
    p.category == TileProviderCategory.global ||
        p.category == TileProviderCategory.local)
        .toList();

    return SingleChildScrollView(
      child: Padding(
        padding: EdgeInsets.fromLTRB(
            24, 24, 24, 24 + MediaQuery.of(context).viewInsets.bottom),
        child: Form(
          key: _formKey,
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Text("Download Details",
                  style: Theme.of(context).textTheme.headlineSmall),
              const SizedBox(height: 24),
              TextFormField(
                controller: _nameController,
                decoration: const InputDecoration(
                    labelText: "Region Name", border: OutlineInputBorder()),
                validator: (v) =>
                v!.trim().isEmpty ? "Name is required" : null,
              ),
              const SizedBox(height: 16),
              if (downloadableProviders.isNotEmpty)
                DropdownButtonFormField<String>(
                  value: _selectedProviderId,
                  decoration: const InputDecoration(
                      labelText: "Map Source", border: OutlineInputBorder()),
                  items: downloadableProviders
                      .map((p) => DropdownMenuItem(
                      value: p.id, child: Text(p.name(context))))
                      .toList(),
                  onChanged: _setSelectedProvider,
                  validator: (v) =>
                  v == null ? 'Please select a map source' : null,
                )
              else
                const Text("No downloadable map sources available."),
              const SizedBox(height: 24),
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
              const SizedBox(height: 16),
              Card(
                color: Theme.of(context).colorScheme.surfaceContainerLow,
                child: Padding(
                  padding: const EdgeInsets.all(12.0),
                  child: Text(
                      "Estimated Tiles: $_tileCount\nEstimated Size: ~$_estimatedSizeMb MB",
                      textAlign: TextAlign.center,
                      style: Theme.of(context).textTheme.bodyMedium),
                ),
              ),
              const SizedBox(height: 24),
              FilledButton.icon(
                style: FilledButton.styleFrom(
                    padding: const EdgeInsets.symmetric(vertical: 16)),
                onPressed: _startDownload,
                icon: const Icon(Icons.download),
                label: const Text("Start Download"),
              )
            ],
          ),
        ),
      ),
    );
  }
}