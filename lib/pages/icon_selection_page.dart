import 'package:flutter/material.dart';
import 'package:map_app/data/model/named_icon.dart';
import '../data/icon.dart';

class IconSelectionPage extends StatefulWidget {
  const IconSelectionPage({super.key});

  @override
  State<IconSelectionPage> createState() => _IconSelectionPageState();
}

class _IconSelectionPageState extends State<IconSelectionPage> {
  final IconService _iconService = IconService();
  late List<NamedIcon> _icons;
  late List<NamedIcon> _filteredIcons;
  final TextEditingController _searchController = TextEditingController();

  final double _itemSize = 64.0; // Total size of each grid item
  final double _iconSize = 32.0; // Size of the icon

  @override
  void initState() {
    super.initState();
    _icons = _iconService.getAllIcons();
    _filteredIcons = _icons;
  }

  void _filterIcons(String query) {
    setState(() {
      _filteredIcons = _icons
          .where((icon) =>
          icon.title.toLowerCase().contains(query.toLowerCase()))
          .toList();
    });
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        leading: IconButton(
          icon: const Icon(Icons.arrow_back),
          onPressed: () => Navigator.pop(context),
        ),
        title: const Text('Ikoner', style: TextStyle(fontWeight: FontWeight.bold),),
        actions: [
          IconButton(
            icon: const Icon(Icons.close),
            onPressed: () => Navigator.pop(context),
          ),
        ],
      ),
      body: Column(
        children: [
          Padding(
            padding: const EdgeInsets.all(16.0),
            child: TextField(
              controller: _searchController,
              decoration: InputDecoration(
                hintText: 'Søk på ikoner',
                prefixIcon: const Icon(Icons.search),
                filled: true,
                fillColor: Colors.grey[200],
                border: OutlineInputBorder(
                  borderRadius: BorderRadius.circular(30),
                  borderSide: BorderSide.none,
                ),
              ),
              onChanged: _filterIcons,
            ),
          ),
          Expanded(
            child: LayoutBuilder(
              builder: (context, constraints) {
                final int crossAxisCount = (constraints.maxWidth / _itemSize).floor();

                return GridView.builder(
                  padding: const EdgeInsets.all(8),
                  gridDelegate: SliverGridDelegateWithFixedCrossAxisCount(
                    crossAxisCount: crossAxisCount,
                    childAspectRatio: 1,
                    crossAxisSpacing: 0,
                    mainAxisSpacing: 0,
                  ),
                  itemCount: _filteredIcons.length,
                  itemBuilder: (context, index) {
                    return MouseRegion(
                      cursor: SystemMouseCursors.click,
                      child: Semantics(
                        button: true,
                        label: 'Select ${_filteredIcons[index].title} icon',
                        child: Material(
                          color: Colors.transparent,
                          child: InkWell(
                            onTap: () {
                              Navigator.pop(context, _filteredIcons[index]);
                            },
                            hoverColor: Colors.blue.withOpacity(0.1),
                            borderRadius: BorderRadius.circular(10),
                            child: Container(
                              width: _itemSize,
                              height: _itemSize,
                              padding: const EdgeInsets.all(4),
                              child: Column(
                                mainAxisAlignment: MainAxisAlignment.center,
                                children: [
                                  Icon(
                                    _filteredIcons[index].icon,
                                    size: _iconSize,
                                    color: Colors.grey[600],
                                  ),
                                  const SizedBox(height: 2),
                                  Text(
                                    _filteredIcons[index].title,
                                    textAlign: TextAlign.center,
                                    style: const TextStyle(fontSize: 10),
                                    maxLines: 1,
                                    overflow: TextOverflow.ellipsis,
                                  ),
                                ],
                              ),
                            ),
                          ),
                        ),
                      ),
                    );
                  },
                );
              },
            ),
          ),
        ],
      ),
    );
  }

  @override
  void dispose() {
    _searchController.dispose();
    super.dispose();
  }
}