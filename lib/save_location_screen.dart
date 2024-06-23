import 'package:flutter/material.dart';
import 'package:latlong2/latlong.dart';
import 'package:provider/provider.dart';
import 'location_provider.dart';

class SaveLocationScreen extends StatefulWidget {
  final LatLng location;

  const SaveLocationScreen({super.key, required this.location});

  @override
  State<SaveLocationScreen> createState() => _SaveLocationScreenState();
}

class _SaveLocationScreenState extends State<SaveLocationScreen> {
  final _formKey = GlobalKey<FormState>();
  final _nameController = TextEditingController();
  final _descriptionController = TextEditingController();

  @override
  void dispose() {
    _nameController.dispose();
    _descriptionController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('Save Location')),
      body: Padding(
        padding: const EdgeInsets.all(16.0),
        child: Form(
          key: _formKey,
          child: Column(
            children: [
              TextFormField(
                controller: _nameController,
                decoration: const InputDecoration(labelText: 'Name'),
                validator: (value) {
                  if (value == null || value.isEmpty) {
                    return 'Please enter a name';
                  }
                  return null;
                },
              ),
              TextFormField(
                controller: _descriptionController,
                decoration: const InputDecoration(labelText: 'Description'),
              ),
              const SizedBox(height: 20),
              ElevatedButton(
                onPressed: () => _saveLocation(context),
                child: const Text('Save'),
              ),
            ],
          ),
        ),
      ),
    );
  }

  Future<void> _saveLocation(BuildContext context) async {
    if (_formKey.currentState!.validate()) {
      await context.read<LocationProvider>().addLocation({
        'name': _nameController.text,
        'description': _descriptionController.text,
        'latitude': widget.location.latitude,
        'longitude': widget.location.longitude,
      });
      if(!context.mounted) return;
      Navigator.of(context).pop();
    }
  }
}