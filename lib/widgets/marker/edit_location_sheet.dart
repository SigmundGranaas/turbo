import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import '../../data/model/marker.dart';
import '../../data/model/named_icon.dart';
import '../../data/icon_service.dart';
import '../../location_provider.dart';
import 'location_base_sheet.dart';

class LocationEditSheet extends StatelessWidget {
  final Marker location;

  const LocationEditSheet({super.key, required this.location});

  @override
  Widget build(BuildContext context) {
    return LocationSheetBase(
      title: 'Rediger markering',
      initialName: location.title,
      initialDescription: location.description,
      initialIcon: IconService().getIcon(location.icon),
      buildButtons: (context, name, description, icon) => Column(
        children: [
          _buildUpdateButton(context, name, description, icon),
          const SizedBox(height: 16),
          _buildDeleteButton(context),
        ],
      ),
    );
  }

  Widget _buildUpdateButton(BuildContext context, String name, String description, NamedIcon icon) {
    return SizedBox(
      width: double.infinity,
      child: ElevatedButton.icon(
        icon: const Icon(Icons.save, color: Colors.white),
        label: const Text('Oppdater', style: TextStyle(color: Colors.white)),
        onPressed: () => _updateLocation(context, name, description, icon),
        style: ElevatedButton.styleFrom(
          backgroundColor: Colors.blueAccent,
          foregroundColor: Colors.white,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(20),
          ),
          padding: const EdgeInsets.symmetric(vertical: 16, horizontal: 12),
        ),
      ),
    );
  }

  Widget _buildDeleteButton(BuildContext context) {
    return SizedBox(
      width: double.infinity,
      child: ElevatedButton.icon(
        icon: const Icon(Icons.delete, color: Colors.white),
        label: const Text('Slett', style: TextStyle(color: Colors.white)),
        onPressed: () => _deleteLocation(context),
        style: ElevatedButton.styleFrom(
          backgroundColor: Colors.redAccent,
          foregroundColor: Colors.white,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(20),
          ),
          padding: const EdgeInsets.symmetric(vertical: 16, horizontal: 12),
        ),
      ),
    );
  }

  Future<void> _updateLocation(BuildContext context, String name, String description, NamedIcon icon) async {
    final locationProvider = context.read<LocationProvider>();
    final updatedMarker = Marker.fromMap({
      ...location.toMap(),
      'title': name,
      'description': description,
      'icon': icon.title,
    });

    try {
      await locationProvider.updateLocation(updatedMarker);
      if (context.mounted) {
        Navigator.of(context).pop(updatedMarker);
      }
    } catch (e) {
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text('Failed to update location: ${e.toString()}'),
            backgroundColor: Colors.red,
          ),
        );
      }
    }
  }

  Future<void> _deleteLocation(BuildContext context) async {
    final locationProvider = context.read<LocationProvider>();

    try {
      await locationProvider.deleteLocation(location.uuid);
      if (context.mounted) {
        Navigator.of(context).pop('deleted');
      }
    } catch (e) {
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text('Failed to delete location: ${e.toString()}'),
            backgroundColor: Colors.red,
          ),
        );
      }
    }
  }
}