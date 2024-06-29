// location_edit_sheet.dart
import 'package:flutter/material.dart';
import 'package:map_app/data/model/named_icon.dart';
import 'package:map_app/pages/icon_selection_page.dart';
import 'package:provider/provider.dart';
import 'package:latlong2/latlong.dart';
import 'data/model/marker.dart';
import 'location_provider.dart';

class LocationEditSheet extends StatefulWidget {
  final Marker? location;
  final LatLng? newLocation;

  const LocationEditSheet({super.key, this.location, this.newLocation});

  @override
  State<LocationEditSheet> createState() => _LocationEditSheetState();
}

class _LocationEditSheetState extends State<LocationEditSheet> {
  final _formKey = GlobalKey<FormState>();
  NamedIcon selectedIcon =  const NamedIcon(title: 'Fjell', icon: Icons.landscape);
  late TextEditingController _nameController;
  late TextEditingController _descriptionController;

  @override
  void initState() {
    super.initState();
    _nameController = TextEditingController(text: widget.location?.title ?? '');
    _descriptionController = TextEditingController(text: widget.location?.description ?? '');
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
                  labelText: 'Navn',
                ),
                validator: (value) {
                  if (value == null || value.isEmpty) {
                    return 'Skriv inn et navn';
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
                    return 'Legg til en beskrivelse!';
                  }
                  return null;
                },
              ),
            ),
            const SizedBox(height: 16),
            Row(
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              children: [
                Text('Ikon', style: TextStyle(fontWeight: FontWeight.bold, color: Colors.grey[600]),)
              ],
            ),
            const SizedBox(height: 4),
            ListTile(
              leading: Icon(selectedIcon.icon),
              title: Text(selectedIcon.title),
              tileColor: Colors.blue.withOpacity(0.1),
              shape: RoundedRectangleBorder(
                borderRadius: BorderRadius.circular(8), // Rounded corners
              ),
              trailing: const Icon(Icons.arrow_forward_ios),
              onTap: () async {
                final NamedIcon? result = await Navigator.push(
                  context,
                  MaterialPageRoute(builder: (context) => const IconSelectionPage()),
                );
                if (result != null) {
                  setState(() {
                    selectedIcon = result;
                  });
                }
              },
            ),
            const SizedBox(height: 32),
            SizedBox(
              width: double.infinity,
              child: ElevatedButton.icon(
                icon: const Icon(Icons.save, color: Colors.white),
                label: const Text('Lagre', style: TextStyle(color: Colors.white)),
                onPressed: () => _saveLocation(context),
                style: ElevatedButton.styleFrom(
                  backgroundColor: Colors.blueAccent,
                  foregroundColor: Colors.white,
                  shape: RoundedRectangleBorder(
                    borderRadius: BorderRadius.circular(20), // Adjust the radius as needed
                  ),
                  padding: const EdgeInsets.symmetric(vertical: 16, horizontal: 12),
                ),
              ),
            ),
            const SizedBox(height: 16),
          ],
        ),
      ),
    );
  }

  Future<void> _saveLocation(BuildContext context) async {
    if (_formKey.currentState!.validate()) {
      final locationProvider = context.read<LocationProvider>();
      final newMarker = Marker.fromMap( {
        'title': _nameController.text,
        'description': _descriptionController.text,
        'icon': selectedIcon.title,
        'latitude': widget.location?.position.latitude ?? widget.newLocation!.latitude,
        'longitude': widget.location?.position.longitude ?? widget.newLocation!.longitude,
      });

      if (widget.location != null) {
        await locationProvider.updateLocation(newMarker);
      } else {
        await locationProvider.addLocation(newMarker);
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
      await context.read<LocationProvider>().deleteLocation(widget.location!.uuid);
      if (!context.mounted) return;
      Navigator.of(context).pop();
    }
  }
}