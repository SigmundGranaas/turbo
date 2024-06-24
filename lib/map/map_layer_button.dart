import 'package:flutter/material.dart';

class MapLayerButton extends StatelessWidget {
  final String currentGlobalLayer;
  final String currentNorwayLayer;
  final Function(String) onBaseLayerChanged;
  final Function(String) onNorwayLayerChanged;

  const MapLayerButton({
    super.key,
    required this.currentGlobalLayer,
    required this.currentNorwayLayer,
    required this.onBaseLayerChanged,
    required this.onNorwayLayerChanged,
  });

  @override
  Widget build(BuildContext context) {
    return FloatingActionButton(
      child: const Icon(Icons.layers),
      onPressed: () {
        showDialog(
          context: context,
          builder: (BuildContext context) {
            String tempGlobalLayer = currentGlobalLayer;
            String tempNorwayLayer = currentNorwayLayer;
            return StatefulBuilder(
              builder: (context, setState) {
                return AlertDialog(
                  title: const Text('Select Map Layers'),
                  content: Column(
                    mainAxisSize: MainAxisSize.min,
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      const Text('Global:', style: TextStyle(fontWeight: FontWeight.bold)),
                      DropdownButton<String>(
                        value: tempGlobalLayer,
                        items: const [
                          DropdownMenuItem(value: 'nothing', child: Text('Nothing')),
                          DropdownMenuItem(value: 'osm', child: Text('OpenStreetMap')),
                        ],
                        onChanged: (value) {
                          setState(() {
                            tempGlobalLayer = value!;
                            onBaseLayerChanged(tempGlobalLayer);
                          });
                        },
                      ),
                      const SizedBox(height: 16),
                      const Text('Norway base layer:', style: TextStyle(fontWeight: FontWeight.bold)),
                      DropdownButton<String>(
                        value: tempNorwayLayer,
                        items: const [
                          DropdownMenuItem(value: 'nothing', child: Text('Nothing')),
                          DropdownMenuItem(value: 'topo', child: Text('Topo')),
                          DropdownMenuItem(value: 'satellite', child: Text('Satellite')),
                        ],
                        onChanged: (value) {
                          setState(() {
                            tempNorwayLayer = value!;
                            onNorwayLayerChanged(tempNorwayLayer);
                          });
                        },
                      ),
                    ],
                  ),
                );
              },
            );
          },
        );
      },
    );
  }
}