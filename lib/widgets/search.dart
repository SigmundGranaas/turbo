import 'package:flutter/material.dart';
import 'package:http/http.dart' as http;
import 'dart:convert';

import '../data/icon_service.dart';

class SearchWidget extends StatefulWidget {
  final Function(double, double) onLocationSelected;

  const SearchWidget({super.key, required this.onLocationSelected});

  @override
  State<SearchWidget> createState() => _SearchWidgetState();
}


class _SearchWidgetState extends State<SearchWidget> {
  final TextEditingController _controller = TextEditingController();
  List<dynamic> _suggestions = [];
  bool _isFocused = false;
  final IconService _iconService = IconService();

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
        builder: (context, constraints) {
          double widgetWidth = constraints.maxWidth > 632 ? 600 : constraints.maxWidth - 32;
          return SizedBox(
            width: widgetWidth,
            child: Card(
              elevation: 6,
              shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(10)),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Padding(
                    padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 4),
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
    final url = Uri.parse('https://api.kartverket.no/stedsnavn/v1/navn?sok=${Uri.encodeComponent(query)}*&fuzzy=true&treffPerSide=5');
    try {
      final response = await http.get(url);
      if (response.statusCode == 200) {
        final data = json.decode(response.body);
        setState(() => _suggestions = data['navn']);
      }
    } catch (e) {
      print('Error fetching suggestions: $e');
    }
  }

  void _onSuggestionSelected(dynamic suggestion) {
    final point = suggestion['representasjonspunkt'];
    final east = point['øst'];
    final north = point['nord'];
    widget.onLocationSelected(east, north);
    setState(() {
      _suggestions = [];
      _isFocused = false;
    });
    _controller.clear();
    FocusScope.of(context).unfocus();
  }

  Widget _buildSuggestionItem(dynamic suggestion) {
    final name = suggestion['skrivemåte'];
    final type = suggestion['navneobjekttype'];
    final municipality = suggestion['kommuner'][0]['kommunenavn'] + " kommune";

    Widget leadingWidget;
    switch (type.toLowerCase()) {
      case 'fjelltopp':
      case 'fjell':
        leadingWidget = Icon(_iconService.getIcon('Fjell').icon, color: Colors.purple[900]);
        break;
      case 'park':
        leadingWidget = Icon(_iconService.getIcon('Park').icon, color: Colors.purple[900]);
        break;
      case 'strand':
        leadingWidget = Icon(_iconService.getIcon('Strand').icon, color: Colors.purple[900]);
        break;
      case 'skog':
        leadingWidget = Icon(_iconService.getIcon('Skog').icon, color: Colors.purple[900]);
        break;
      default:
        leadingWidget = Text(
          name[0].toUpperCase(),
          style: TextStyle(color: Colors.purple[900], fontWeight: FontWeight.bold),
        );
    }

    return ListTile(
      leading: CircleAvatar(
        backgroundColor: Colors.purple[100],
        child: leadingWidget,
      ),
      title: Text(name),
      subtitle: Text('$type i $municipality'),
      onTap: () => _onSuggestionSelected(suggestion),
    );
  }
}