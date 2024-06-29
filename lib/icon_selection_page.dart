import 'package:flutter/material.dart';
import 'package:map_app/data/model/named_icon.dart';

class IconSelectionPage extends StatefulWidget {
  const IconSelectionPage({super.key});

  @override
  State<IconSelectionPage> createState() => _IconSelectionPageState();
}


class _IconSelectionPageState extends State<IconSelectionPage> {
  final NamedIcon fjell = NamedIcon(title: 'Fjell', icon: Icons.landscape);

  final List<NamedIcon> iconList = [
    NamedIcon(title: 'Fjell', icon: Icons.landscape),
     NamedIcon(title: 'Park', icon: Icons.park),
     NamedIcon(title: 'Strand', icon: Icons.beach_access),
     NamedIcon(title: 'Skog', icon: Icons.forest),
     NamedIcon(title: 'Vandring', icon: Icons.hiking),
     NamedIcon(title: 'Kajakk', icon: Icons.kayaking),
    // Add more NamedIcon instances as needed
  ];

  List<NamedIcon> filteredIcons = [];
  bool isSearching = true;

  @override
  void initState() {
    super.initState();
    filteredIcons = List.from(iconList);
  }

  void _filterIcons(String query) {
    setState(() {
      filteredIcons = iconList
          .where((icon) => icon.title.toLowerCase().contains(query.toLowerCase()))
          .toList();
    });
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: isSearching
            ? TextField(
          autofocus: true,
          decoration: const InputDecoration(
            hintText: 'Søk etter ikon...',
            border: InputBorder.none,
          ),
          onChanged: _filterIcons,
        )
            : const Text('Velg ikon'),
        actions: [
          IconButton(
            icon: Icon(isSearching ? Icons.close : Icons.search),
            onPressed: () {
              setState(() {
                isSearching = !isSearching;
                if (!isSearching) {
                  filteredIcons = List.from(iconList);
                }
              });
            },
          ),
        ],
      ),
      body: GridView.builder(
        padding: const EdgeInsets.all(16),
        gridDelegate: const SliverGridDelegateWithFixedCrossAxisCount(
          crossAxisCount: 4,
          crossAxisSpacing: 16,
          mainAxisSpacing: 16,
        ),
        itemCount: filteredIcons.length,
        itemBuilder: (context, index) {
          return InkWell(
            onTap: () {
              Navigator.pop(context, filteredIcons[index]);
            },
            child: Column(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                Container(
                  padding: const EdgeInsets.all(8),
                  decoration: BoxDecoration(
                    border: Border.all(color: Colors.grey),
                    borderRadius: BorderRadius.circular(8),
                  ),
                  child: Icon(filteredIcons[index].icon, size: 32),
                ),
                const SizedBox(height: 4),
                Text(
                  filteredIcons[index].title,
                  style: const TextStyle(fontSize: 12),
                  textAlign: TextAlign.center,
                  overflow: TextOverflow.ellipsis,
                ),
              ],
            ),
          );
        },
      ),
    );
  }
}