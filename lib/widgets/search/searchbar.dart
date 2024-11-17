import 'package:flutter/material.dart';
import 'package:map_app/data/search/location_service.dart';

import '../../data/icon_service.dart';

class SearchWidget extends StatefulWidget {
  final Function(double, double) onLocationSelected;
  final LocationService service;

  const SearchWidget({super.key, required this.onLocationSelected, required this.service});

  @override
  State<SearchWidget> createState() => _SearchWidgetState();
}

class _SearchWidgetState extends State<SearchWidget> {
  final TextEditingController _controller = TextEditingController();
  List<LocationSearchResult> _suggestions = [];
  bool _isFocused = false;
  final IconService _iconService = IconService();

  @override
  Widget build(BuildContext context) {
    final width = MediaQuery.of(context).size.width;
    final isMobile = width < 600;

    final horizontal = isMobile ? 12.0 : 16.0;
    final vertical = isMobile ? 0.0 : 4.0;

    return LayoutBuilder(
        builder: (context, constraints) {
          double widgetWidth = constraints.maxWidth > 632 ? 600 : constraints.maxWidth - 32;
          return SizedBox(
            width: widgetWidth,
            child: Card(
              elevation: 4,
              shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(25)),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Padding(
                    padding: EdgeInsets.symmetric(horizontal: horizontal, vertical: vertical),
                    child: Row(
                      children: [
                        Icon(Icons.search, color: Colors.grey[600]),
                        const SizedBox(width: 8),
                        Expanded(
                          child: TextField(
                            controller: _controller,
                            decoration: const InputDecoration(
                              hintText: 'Søk her',
                              border: InputBorder.none,
                            ),
                            onChanged: _onSearchChanged,
                            onTap: () {
                              setState(() => _isFocused = true);
                            },
                          ),
                        ),
                        if (_isFocused)
                          IconButton(
                            icon: const Icon(Icons.close),
                            onPressed: () {
                              _controller.clear();
                              setState(() {
                                _suggestions = [];
                                _isFocused = false;
                              });
                              FocusScope.of(context).unfocus();
                            },
                          ),
                      ],
                    ),
                  ),
                  if (_suggestions.isNotEmpty && _isFocused)
                    Container(
                      constraints: BoxConstraints(
                        maxHeight: MediaQuery.of(context).size.height * 0.3,
                      ),
                      child: ListView.builder(
                        shrinkWrap: true,
                        itemCount: _suggestions.length,
                        itemBuilder: (context, index) => _buildSuggestionItem(_suggestions[index]),
                      ),
                    ),
                ],
              ),
            ),
          );
        }
    );
  }

  void _onSearchChanged(String query) {
    if (query.length < 2) {
      setState(() => _suggestions = []);
      return;
    }
    _fetchSuggestions(query);
  }

  void _fetchSuggestions(String query) async {
    try {
      final data = await widget.service.findLocationsBy(query);
      setState(() => _suggestions = data);
    } catch (e) {
      // print('Error fetching suggestions: $e');
    }
  }

  void _onSuggestionSelected(LocationSearchResult suggestion) {
    widget.onLocationSelected(suggestion.position.latitude, suggestion.position.longitude);
    setState(() {
      _suggestions = [];
      _isFocused = false;
    });
    _controller.clear();
    FocusScope.of(context).unfocus();
  }

  Widget _leadingWidget(LocationSearchResult suggestion){
    if(suggestion.icon != null){
      return Icon(_iconService.getIcon(suggestion.icon).icon);
    }else{
      return Text(
        suggestion.title[0].toUpperCase(),
        style: TextStyle(color: Colors.purple[900], fontWeight: FontWeight.bold),
      );
    }
  }

  Widget _buildSuggestionItem(LocationSearchResult suggestion) {
    return ListTile(
      leading: CircleAvatar(
        backgroundColor: Colors.purple[100],
        child: _leadingWidget(suggestion),
      ),
      title: Text(suggestion.title),
      subtitle: Text(suggestion.description ?? ''),
      onTap: () => _onSuggestionSelected(suggestion),
    );
  }
}