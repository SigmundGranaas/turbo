import 'package:flutter/material.dart';
import 'package:idb_shim/idb.dart';
import 'package:map_app/data/model/named_icon.dart';
import 'package:provider/provider.dart';
import 'package:latlong2/latlong.dart';
import '../../data/icon_service.dart';
import '../../data/model/marker.dart';
import '../../location_provider.dart';
import '../pages/icon_selection_page.dart';
import 'location_base_sheet.dart';

class CreateLocationSheet extends StatefulWidget {
  final Marker? location;
  final LatLng? newLocation;

  const CreateLocationSheet({super.key, this.location, this.newLocation});

  @override
  State<CreateLocationSheet> createState() => _CreateLocationSheetState();
}

class _CreateLocationSheetState extends State<CreateLocationSheet> {
  final _formKey = GlobalKey<FormState>();
  late NamedIcon _selectedIcon;
  late TextEditingController _nameController;
  late TextEditingController _descriptionController;

  @override
  void initState() {
    super.initState();
    _selectedIcon = const NamedIcon(title: 'Fjell', icon: Icons.landscape);
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
            _buildHeader(),
            LocationFormFields(
              nameController: _nameController,
              descriptionController: _descriptionController,
              selectedIcon: _selectedIcon,
              onIconSelected: (icon) => setState(() => _selectedIcon = icon),
            ),
            const SizedBox(height: 32),
            _buildSaveButton(),
            const SizedBox(height: 16),
          ],
        ),
      ),
    );
  }

  Widget _buildHeader() {
    return Padding(
      padding: const EdgeInsets.only(bottom: 16.0),
      child: Row(
        mainAxisAlignment: MainAxisAlignment.spaceBetween,
        children: [
          Text(
            'Ny markering',
            style: TextStyle(fontWeight: FontWeight.bold, color: Colors.grey[600]),
          ),
          IconButton(
            onPressed: () => Navigator.pop(context),
            icon: const Icon(Icons.close),
          )
        ],
      ),
    );
  }

  Widget _buildSaveButton() {
    return SizedBox(
      width: double.infinity,
      child: ElevatedButton.icon(
        icon: const Icon(Icons.save, color: Colors.white),
        label: const Text('Lagre', style: TextStyle(color: Colors.white)),
        onPressed: () => _saveLocation(context),
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

  Future<void> _saveLocation(BuildContext context) async {
    if (_formKey.currentState!.validate()) {
      final locationProvider = context.read<LocationProvider>();
      final newMarker = Marker.fromMap({
        'title': _nameController.text,
        'description': _descriptionController.text,
        'icon': _selectedIcon.title,
        'latitude': widget.location?.position.latitude ?? widget.newLocation!.latitude,
        'longitude': widget.location?.position.longitude ?? widget.newLocation!.longitude,
      });

      try {
        await locationProvider.addLocation(newMarker);

        if (!context.mounted) return;

        Navigator.of(context).pop(newMarker);
      } catch (e) {
        // Handle the error
        if (!context.mounted) return;

        Navigator.of(context).pop();

        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text(_getErrorMessage(e)),
            backgroundColor: Colors.red,
          ),
        );
      }
    }
  }

  String _getErrorMessage(dynamic error) {
  if (error is DatabaseException) {
      return 'Database error. Please try again later.';
    } else {
      return 'An unexpected error occurred. Please try again. ${error.toString()}';
    }
  }
}