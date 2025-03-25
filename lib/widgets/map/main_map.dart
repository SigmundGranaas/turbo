import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:latlong2/latlong.dart';
import 'package:map_app/widgets/auth/drawer_widget.dart';
import 'package:map_app/data/search/marker_search_service.dart';
import 'package:map_app/widgets/map/measuring/measuring_map.dart';

import '../../data/datastore/factory.dart';
import '../../data/icon_service.dart';
import '../../data/model/marker.dart' as marker_model;
import '../../data/search/composite_search_service.dart';
import '../../data/search/kartverket_location_service.dart';
import '../../data/search/location_service.dart';
import '../marker/create_location_sheet.dart';
import '../search/searchbar.dart';
import 'controller/map_utility.dart';
import 'controller/provider/map_controller.dart';
import 'controls/default_map_controls.dart';
import 'controls/map_controls.dart';
import 'layers/current_location_layer.dart';
import 'layers/saved_markers_layer.dart';
import 'layers/tiles/tile_registry/tile_registry.dart';
import 'map_base.dart';

class MapControllerPage extends ConsumerStatefulWidget {
  const MapControllerPage({super.key});

  @override
  ConsumerState<MapControllerPage> createState() => MapControllerPageState();
}

class MapControllerPageState extends ConsumerState<MapControllerPage>
    with TickerProviderStateMixin {
  late MapController _mapController;
  Marker? _temporaryPin;
  final GlobalKey<ScaffoldState> _scaffoldKey = GlobalKey<ScaffoldState>();

  @override
  Widget build(BuildContext context) {
    final layers = ref.watch(tileRegistryProvider.notifier).getActiveLayers();
    final registry = ref.watch(tileRegistryProvider);
    _mapController = ref.watch(mapControllerProvProvider.notifier).controller();

    final mapLayers = [
      ...layers,
      ...registry.activeGlobalIds.map((id) =>
          RichAttributionWidget(
            animationConfig: const ScaleRAWA(),
            attributions: [
              TextSourceAttribution(
                  registry.availableProviders[id]!.attributions),
            ],
          )),
      ...registry.activeLocalIds.map((id) =>
          RichAttributionWidget(
            animationConfig: const ScaleRAWA(),
            attributions: [
              TextSourceAttribution(
                  registry.availableProviders[id]!.attributions),
            ],
          )),
      const CurrentLocationLayer(),
      LocationMarkers(
        onMarkerTap: (location) => _showEditSheet(context, location),
      ),
      if (_temporaryPin != null) MarkerLayer(markers: [_temporaryPin!]),
    ];

    final width = MediaQuery.of(context).size.width;
    final isMobile = width < 600;
    final List<Widget> controls;

    if(isMobile){
      controls = defaultMobileMapControls(_mapController, this);
    }else{
      controls = defaultMapControls(_mapController, this);
    }

    final colorScheme = Theme.of(context).colorScheme;

    return Scaffold(
      key: _scaffoldKey,
      drawer: const AppDrawer(), // The drawer remains
      body: Stack(
        children: [
          // Base map with layers
          MapBase(
            mapController: _mapController,
            mapLayers: mapLayers,
            overlayWidgets: [
              MapControls(controls: controls),
              // Search bar positioned at the top
              Positioned(
                top: 20,
                left: 0,
                right: 0,
                child: Center(
                  child: _buildCustomSearchBar(isMobile),
                ),
              ),
            ],
            onLongPress: (tapPosition, point) => _handleLongPress(context, point),
          ),

          // Desktop circular menu button in top left when not mobile
          if (!isMobile)
            Positioned(
              left: 20,
              top: 20,
              child: SizedBox(
                width: 64,
                height: 64,
                child: Card(
                  elevation: 4,
                  shape: const CircleBorder(),
                  child: ClipOval(
                    child: Material(
                      color: Theme.of(context).colorScheme.surface,
                      child: InkWell(
                        onTap: () {
                          _scaffoldKey.currentState?.openDrawer();
                        },
                        child: Padding(
                          padding: const EdgeInsets.all(8),
                          child: Icon(Icons.menu, color: colorScheme.primary),
                        ),
                      ),
                    ),
                  ),
                ),
              ),
            ),
        ],
      ),
    );
  }

  // Custom search bar with menu icon inside for mobile
  Widget _buildCustomSearchBar(bool isMobile) {
    return LayoutBuilder(
        builder: (context, constraints) {
          double widgetWidth = constraints.maxWidth > 632 ? 600 : constraints.maxWidth - 32;

          if (isMobile) {
            // On mobile: Custom search widget with menu icon inside
            return CustomSearchWidget(
              width: widgetWidth,
              onLocationSelected: (double east, double north) {
                animatedMapMove(LatLng(north, east), 13, _mapController, this);
              },
              service: CompositeSearchService(
                  KartverketLocationService(),
                  MarkerSearchService(MarkerDataStoreFactory.getDataStore())
              ),
              onMenuPressed: () {
                _scaffoldKey.currentState?.openDrawer();
              },
            );
          } else {
            // On desktop: Just the standard search bar (menu button is separate)
            return SearchWidget(
              onLocationSelected: (double east, double north) {
                animatedMapMove(LatLng(north, east), 13, _mapController, this);
              },
              service: CompositeSearchService(
                  KartverketLocationService(),
                  MarkerSearchService(MarkerDataStoreFactory.getDataStore())
              ),
            );
          }
        }
    );
  }

  void _handleLongPress(BuildContext context, LatLng point) {
    setState(() {
      _temporaryPin = Marker(
        point: point,
        child: const Icon(Icons.location_pin, color: Colors.red, size: 30),
      );
    });

    _showPinOptionsSheet(context, point);
  }

  void _showPinOptionsSheet(BuildContext context, LatLng point) {
    showModalBottomSheet(
      context: context,
      builder: (BuildContext context) {
        return Container(
          padding: const EdgeInsets.all(16),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              ListTile(
                leading: const Icon(Icons.save),
                title: const Text('Save Location'),
                onTap: () {
                  Navigator.pop(context);
                  _showEditSheet(context, null, newLocation: point);
                },
              ),
              ListTile(
                leading: const Icon(Icons.straighten),
                title: const Text('Start Measuring'),
                onTap: () {
                  Navigator.pop(context);
                  _navigateToMeasuring(point);
                },
              ),
            ],
          ),
        );
      },
    ).whenComplete(() {
      setState(() {
        _temporaryPin = null;
      });
    });
  }

  void _navigateToMeasuring(LatLng startPoint) {
    Navigator.of(context).push(
      MaterialPageRoute(
        builder: (context) => MeasuringControllerPage(
          initialPosition: _mapController.camera.center,
          startPoint: startPoint,
          zoom: _mapController.camera.zoom,
        ),
      ),
    );
  }

  void _showEditSheet(BuildContext context, marker_model.Marker? marker,
      {LatLng? newLocation}) async {
    if (newLocation != null) {
      animatedMapMove(
          newLocation,
          _mapController.camera.zoom,
          _mapController,
          this
      );
    }

    await showModalBottomSheet(
      context: context,
      isScrollControlled: true,
      builder: (BuildContext context) {
        return CreateLocationSheet(
            location: marker,
            newLocation: newLocation);
      },
    );
  }
}

// Custom search widget with integrated menu button
class CustomSearchWidget extends StatefulWidget {
  final Function(double, double) onLocationSelected;
  final LocationService service;
  final VoidCallback onMenuPressed;
  final double width;

  const CustomSearchWidget({
    super.key,
    required this.onLocationSelected,
    required this.service,
    required this.onMenuPressed,
    required this.width,
  });

  @override
  State<CustomSearchWidget> createState() => _CustomSearchWidgetState();
}

class _CustomSearchWidgetState extends State<CustomSearchWidget> {
  final TextEditingController _controller = TextEditingController();
  List<LocationSearchResult> _suggestions = [];
  bool _isFocused = false;
  final IconService _iconService = IconService();

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: widget.width,
      child: Card(
        elevation: 4,
        color: Theme.of(context).colorScheme.surface,
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(25)),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 12.0, vertical: 2.0),
              child: Row(
                children: [
                  // Menu icon inside the search bar
                  IconButton(
                    icon: Icon(Icons.menu, color: Theme.of(context).colorScheme.primary),
                    onPressed: widget.onMenuPressed,
                  ),
                  // Search icon
                  const SizedBox(width: 8),
                  Expanded(
                    child: TextField(
                      controller: _controller,

                      decoration: const InputDecoration(
                        hintText: 'Søk her',
                        border: InputBorder.none,
                      ),
                      onChanged: _onSearchChanged,
                      onTap: () {
                        setState(() => _isFocused = true);
                      },
                    ),
                  ),
                  if (_isFocused)
                    IconButton(
                      icon: const Icon(Icons.close),
                      onPressed: () {
                        _controller.clear();
                        setState(() {
                          _suggestions = [];
                          _isFocused = false;
                        });
                        FocusScope.of(context).unfocus();
                      },
                    ),
                ],
              ),
            ),
            if (_suggestions.isNotEmpty && _isFocused)
              Container(
                constraints: BoxConstraints(
                  maxHeight: MediaQuery.of(context).size.height * 0.3,
                ),
                child: ListView.builder(
                  shrinkWrap: true,
                  itemCount: _suggestions.length,
                  itemBuilder: (context, index) => _buildSuggestionItem(_suggestions[index]),
                ),
              ),
          ],
        ),
      ),
    );
  }

  void _onSearchChanged(String query) {
    if (query.length < 2) {
      setState(() => _suggestions = []);
      return;
    }
    _fetchSuggestions(query);
  }

  void _fetchSuggestions(String query) async {
    try {
      final data = await widget.service.findLocationsBy(query);
      setState(() => _suggestions = data);
    } catch (e) {
      // print('Error fetching suggestions: $e');
    }
  }

  void _onSuggestionSelected(LocationSearchResult suggestion) {
    widget.onLocationSelected(suggestion.position.longitude, suggestion.position.latitude);
    setState(() {
      _suggestions = [];
      _isFocused = false;
    });
    _controller.clear();
    FocusScope.of(context).unfocus();
  }

  Widget _leadingWidget(LocationSearchResult suggestion){
    if(suggestion.icon != null){
      return Icon(_iconService.getIcon(suggestion.icon).icon);
    }else{
      return Text(
        suggestion.title[0].toUpperCase(),
        style: TextStyle(color: Colors.purple[900], fontWeight: FontWeight.bold),
      );
    }
  }

  Widget _buildSuggestionItem(LocationSearchResult suggestion) {
    return ListTile(
      leading: CircleAvatar(
        backgroundColor: Colors.purple[100],
        child: _leadingWidget(suggestion),
      ),
      title: Text(suggestion.title),
      subtitle: Text(suggestion.description ?? ''),
      onTap: () => _onSuggestionSelected(suggestion),
    );
  }
}