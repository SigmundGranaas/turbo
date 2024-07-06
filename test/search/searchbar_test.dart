import 'package:flutter_test/flutter_test.dart';
import 'package:latlong2/latlong.dart';
import 'package:map_app/data/search/location_service.dart';

void main() {
  late MockLocationSearchService mock;

  setUp(() {
    mock = MockLocationSearchService();
  });

  testWidgets('Searching for a location gives back results', (WidgetTester tester) async {

  });
}

class MockLocationSearchService extends LocationService {
  final Map<String, LocationSearchResult> _results = {
    'Heggmotind': LocationSearchResult(icon: 'fjell', title: 'Rago', description: 'Fjell i Bodø kommune', position: LatLng(0, 0)),
    'Bodø': LocationSearchResult(icon: 'city', title: 'Bodø', description: 'By i Nordland', position:  LatLng(50, 50)),
    'Rago':  LocationSearchResult(icon: 'fjell', title: 'Rago', description: 'Fjell i Rago Nasjonalpark', position:  LatLng(50, 50)),
  };

  @override
  Future<List<LocationSearchResult>> findLocationsBy(String name) {
    if(_results.containsKey(name)){
      return  Future.value([_results[name]!]);
    }else{
      return Future.value(List.empty());
    }
  }
}