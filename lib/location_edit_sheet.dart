// location_edit_sheet.dart
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:latlong2/latlong.dart';
import 'location_provider.dart';

class LocationEditSheet extends StatefulWidget {
  final Map<String, dynamic>? location;
  final LatLng? newLocation;

  const LocationEditSheet({super.key, this.location, this.newLocation});

  @override
  State<LocationEditSheet> createState() => _LocationEditSheetState();
}

class _LocationEditSheetState extends State<LocationEditSheet> {
  final _formKey = GlobalKey<FormState>();
  late TextEditingController _nameController;
  late TextEditingController _descriptionController;

  @override
  void initState() {
    super.initState();
    _nameController = TextEditingController(text: widget.location?['name'] ?? '');
    _descriptionController = TextEditingController(text: widget.location?['description'] ?? '');
  }

  @override
  void dispose() {
    _nameController.dispose();
    _descriptionController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: EdgeInsets.only(
        bottom: MediaQuery.of(context).viewInsets.bottom,
        left: 16,
        right: 16,
        top: 16,
      ),
      child: Form(
        key: _formKey,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Padding(padding: const EdgeInsets.only(bottom: 32.0),
              child:  Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: [
                  Text('Rediger markering', style: TextStyle(fontWeight: FontWeight.bold, color: Colors.grey[600]),),
                  const Icon(Icons.close)
                ],
              ),
            ),
             Padding(padding: const  EdgeInsets.only(bottom: 16.0),
              child: TextFormField(
                controller: _nameController,
                decoration: const InputDecoration(
                  border: OutlineInputBorder(),
                  labelText: 'Enter a name',
                ),
                validator: (value) {
                  if (value == null || value.isEmpty) {
                    return 'Please enter a name';
                  }
                  return null;
                },
              ),
            ),
            Padding(padding: const  EdgeInsets.only(bottom: 16.0),
              child: TextFormField(
                controller: _descriptionController,
                decoration: const InputDecoration(
                  border: OutlineInputBorder(),
                  labelText: 'Beskrivelse',
                ),
                validator: (value) {
                  if (value == null || value.isEmpty) {
                    return 'Please enter a name';
                  }
                  return null;
                },
              ),
            ),
            Padding(padding: const  EdgeInsets.only(bottom: 16.0),
              child: Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: [
                  ElevatedButton(
                    onPressed: () => _saveLocation(context),
                    child: Text(widget.location != null ? 'Update' : 'Save'),
                  ),
                  if (widget.location != null) ...[
                    ElevatedButton(
                      onPressed: () => _deleteLocation(context),
                      style: ElevatedButton.styleFrom(backgroundColor: Colors.red),
                      child: const Text('Slett', style: TextStyle(fontWeight: FontWeight.bold, color: Colors.white),),
    ),
                  ],
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }

  Future<void> _saveLocation(BuildContext context) async {
    if (_formKey.currentState!.validate()) {
      final locationProvider = context.read<LocationProvider>();
      final newLocation = {
        'name': _nameController.text,
        'description': _descriptionController.text,
        'latitude': widget.location?['latitude'] ?? widget.newLocation!.latitude,
        'longitude': widget.location?['longitude'] ?? widget.newLocation!.longitude,
      };

      if (widget.location != null) {
        newLocation['id'] = widget.location!['id'];
        await locationProvider.updateLocation(newLocation);
      } else {
        await locationProvider.addLocation(newLocation);
      }
      if (!context.mounted) return;
      Navigator.of(context).pop();
    }
  }

  Future<void> _deleteLocation(BuildContext context) async {
    bool confirm = await showDialog(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Confirm Delete'),
        content: const Text('Are you sure you want to delete this location?'),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: const Text('Cancel'),
          ),
          TextButton(
            onPressed: () => Navigator.of(context).pop(true),
            child: const Text('Delete'),
          ),
        ],
      ),
    );

    if (confirm) {
      if (!context.mounted) return;
      await context.read<LocationProvider>().deleteLocation(widget.location!['id']);
      if (!context.mounted) return;
      Navigator.of(context).pop();
    }
  }
}