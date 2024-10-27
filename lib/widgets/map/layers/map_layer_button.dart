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
    return Card(
      elevation: 4,
      child: Padding(
        padding: const EdgeInsets.all(8),
        child: IconButton(
          icon: const Icon(Icons.layers),
          onPressed: () => _showBottomSheet(context),
        ),
      ),
    );
  }

  void _showBottomSheet(BuildContext context) {
    showModalBottomSheet(
      context: context,
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(top: Radius.circular(20)),
      ),
      builder: (BuildContext context) {
        return _LayerSelectionSheet(
          currentGlobalLayer: currentGlobalLayer,
          currentNorwayLayer: currentNorwayLayer,
          onBaseLayerChanged: onBaseLayerChanged,
          onNorwayLayerChanged: onNorwayLayerChanged,
        );
      },
    );
  }
}

class _LayerSelectionSheet extends StatefulWidget {
  final String currentGlobalLayer;
  final String currentNorwayLayer;
  final Function(String) onBaseLayerChanged;
  final Function(String) onNorwayLayerChanged;

  const _LayerSelectionSheet({
    required this.currentGlobalLayer,
    required this.currentNorwayLayer,
    required this.onBaseLayerChanged,
    required this.onNorwayLayerChanged,
  });

  @override
  _LayerSelectionSheetState createState() => _LayerSelectionSheetState();
}

class _LayerSelectionSheetState extends State<_LayerSelectionSheet> {
  late String tempGlobalLayer;
  late String tempNorwayLayer;

  @override
  void initState() {
    super.initState();
    tempGlobalLayer = widget.currentGlobalLayer;
    tempNorwayLayer = widget.currentNorwayLayer;
  }

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(16),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              const Text('Velg kartlag',
                  style: TextStyle(fontSize: 18, fontWeight: FontWeight.bold)),
              IconButton(
                icon: const Icon(Icons.close),
                onPressed: () => Navigator.of(context).pop(),
              ),
            ],
          ),
          const SizedBox(height: 16),
          const Text('Globalt', style: TextStyle(fontWeight: FontWeight.bold)),
          const SizedBox(height: 8),

          Row(
            children: [
              Container(
                child: _buildLayerCard('OSM', 'osm', tempGlobalLayer, (value) {
                  setState(() {
                    tempGlobalLayer = value ? 'osm' : 'nothing';
                    widget.onBaseLayerChanged(tempGlobalLayer);
                  });
                }),
              ),
              Container(
                child: _buildLayerCard('Google Satellite', 'gs', tempGlobalLayer, (value) {
                  setState(() {
                    tempGlobalLayer = value ? 'gs' : 'nothing';
                    widget.onBaseLayerChanged(tempGlobalLayer);
                  });
                }),
              ),
            ],
          ),
          const SizedBox(height: 16),
          const Text('Norge', style: TextStyle(fontWeight: FontWeight.bold)),
          const SizedBox(height: 8),
          Row(
            children: [
              Container(
                child: _buildLayerCard('Topografisk', 'topo', tempNorwayLayer,
                    (value) {
                  setState(() {
                    tempNorwayLayer = value ? 'topo' : 'nothing';
                    widget.onNorwayLayerChanged(tempNorwayLayer);
                  });
                }),
              ),
              const SizedBox(width: 8),
              Container(
                child: _buildLayerCard('Satelitt', 'satellite', tempNorwayLayer,
                    (value) {
                  setState(() {
                    tempNorwayLayer = value ? 'satellite' : 'nothing';
                    widget.onNorwayLayerChanged(tempNorwayLayer);
                  });
                }),
              ),
            ],
          ),
        ],
      ),
    );
  }

  Widget _buildLayerCard(String label, String value, String currentValue,
      Function(bool) onChanged) {
    bool isSelected = currentValue == value;
    return  Column(
        children: [
          Card(
            elevation: 2,
            color: isSelected ? Colors.blue.shade100 : Colors.white,
            child:  Padding(
              padding: const EdgeInsets.all(8.0),
              child: IconButton(onPressed: () => onChanged(!isSelected), icon: const Icon(Icons.layers)),
            ),
          ),
          Padding(
            padding: const EdgeInsets.all(8.0),
            child: Text(label,
                style: const TextStyle(fontWeight: FontWeight.bold)),
          ),
        ],
    );
  }
}
