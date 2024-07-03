import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import '../../data/model/marker.dart';
import '../../data/model/named_icon.dart';
import '../../data/icon_service.dart';
import '../../location_provider.dart';
import 'location_base_sheet.dart';

class EditLocationSheet extends StatefulWidget {
  final Marker location;

  const EditLocationSheet({super.key, required this.location});

  @override
  EditLocationSheetState createState() => EditLocationSheetState();
}

class EditLocationSheetState extends State<EditLocationSheet> {
  final _formKey = GlobalKey<FormState>();
  late final TextEditingController _nameController;
  late final TextEditingController _descriptionController;
  late NamedIcon _selectedIcon;

  @override
  void initState() {
    super.initState();
    _nameController = TextEditingController(text: widget.location.title);
    _descriptionController =
        TextEditingController(text: widget.location.description);
    _selectedIcon = IconService().getIcon(widget.location.icon);
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
            Padding(
              padding: const EdgeInsets.only(bottom: 16.0),
              child: Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: [
                  Text(
                    'Ny markering',
                    style: TextStyle(
                        fontWeight: FontWeight.bold, color: Colors.grey[600]),
                  ),
                  IconButton(
                    onPressed: () => Navigator.pop(context),
                    icon: const Icon(Icons.close),
                  )
                ],
              ),
            ),
            const SizedBox(height: 16),
            LocationFormFields(
              nameController: _nameController,
              descriptionController: _descriptionController,
              selectedIcon: _selectedIcon,
              onIconSelected: (icon) => setState(() => _selectedIcon = icon),
            ),
            const SizedBox(height: 32),
            Row(
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              children: [
                SizedBox(
                  width: 128,
                  child: ElevatedButton.icon(
                    icon: const Icon(Icons.delete, color: Colors.white),
                    label: const Text('Slett',
                        style: TextStyle(color: Colors.white)),
                    onPressed: _deleteLocation,
                    style: ElevatedButton.styleFrom(
                      backgroundColor: Colors.redAccent,
                      foregroundColor: Colors.white,
                      shape: RoundedRectangleBorder(
                        borderRadius: BorderRadius.circular(20),
                      ),
                      padding: const EdgeInsets.symmetric(
                          vertical: 16, horizontal: 12),
                    ),
                  ),
                ),
                SizedBox(
                  width: 128,
                  child: ElevatedButton.icon(
                    icon: const Icon(Icons.save, color: Colors.white),
                    label: const Text('Lagre',
                        style: TextStyle(color: Colors.white)),
                    onPressed: _updateLocation,
                    style: ElevatedButton.styleFrom(
                      backgroundColor: Colors.blueGrey,
                      foregroundColor: Colors.white,
                      shape: RoundedRectangleBorder(
                        borderRadius: BorderRadius.circular(20),
                      ),
                      padding: const EdgeInsets.symmetric(
                          vertical: 16, horizontal: 12),
                    ),
                  ),
                ),
              ],
            ),
            const SizedBox(height: 16),
          ],
        ),
      ),
    );
  }

  void _updateLocation() {
    if (_formKey.currentState!.validate()) {
      final locationProvider =
          Provider.of<LocationProvider>(context, listen: false);
      final updatedMarker = Marker.fromMap({
        ...widget.location.toMap(),
        'title': _nameController.text,
        'description': _descriptionController.text,
        'icon': _selectedIcon.title,
      });

      locationProvider.updateLocation(updatedMarker).then((_) {
        Navigator.of(context).pop();
      }).catchError((error) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Error updating location: $error')),
        );
      });
    }
  }

  void _deleteLocation() {
    final locationProvider =
        Provider.of<LocationProvider>(context, listen: false);
    locationProvider.deleteLocation(widget.location.uuid).then((_) {
      Navigator.of(context).pop();
    }).catchError((error) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Error deleting location: $error')),
      );
    });
  }
}
