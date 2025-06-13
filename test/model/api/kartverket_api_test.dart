import 'package:flutter_test/flutter_test.dart';
import 'package:turbo/data/search/api/kartverket_stedsnavn.dart';
import 'package:turbo/data/search/kartverket_location_service.dart';

void main() {
  group('KartverketLocationService', () {
    late KartverketLocationService service;

    setUp(() {
      service = KartverketLocationService();
    });

    test('convertResponsesToLocationResults parses API response correctly', () {
      final apiResponse = KartverketApiResponse.fromJson({
        "metadata": {
          "side": 1,
          "sokeStreng": "sok=heggmo&utkoordsys=4258&treffPerSide=10&side=1",
          "totaltAntallTreff": 2,
          "treffPerSide": 10,
          "viserFra": 1,
          "viserTil": 10
        },
        "navn": [
          {
            "fylker": [
              {
                "fylkesnavn": "Nordland",
                "fylkesnummer": "18"
              }
            ],
            "kommuner": [
              {
                "kommunenavn": "Hemnes",
                "kommunenummer": "1832"
              }
            ],
            "navneobjekttype": "Bruk",
            "navnestatus": "hovednavn",
            "representasjonspunkt": {
              "koordsys": 4258,
              "nord": 65.90951,
              "øst": 14.26984
            },
            "skrivemåte": "Heggmoen",
            "skrivemåtestatus": "godkjent og prioritert",
            "språk": "Norsk",
            "stedsnummer": 865096,
            "stedstatus": "aktiv"
          },
          {
            "fylker": [
              {
                "fylkesnavn": "Nordland",
                "fylkesnummer": "18"
              }
            ],
            "kommuner": [
              {
                "kommunenavn": "Beiarn",
                "kommunenummer": "1839"
              }
            ],
            "navneobjekttype": "Bruk",
            "navnestatus": "hovednavn",
            "representasjonspunkt": {
              "koordsys": 4258,
              "nord": 66.77537,
              "øst": 14.58793
            },
            "skrivemåte": "Heggmo",
            "skrivemåtestatus": "godkjent og prioritert",
            "språk": "Norsk",
            "stedsnummer": 866372,
            "stedstatus": "aktiv"
          }
        ]
      });

      final results = service.convertResponsesToLocationResults(apiResponse.navn);

      expect(results.length, 2);
      expect(results[0].title, 'Heggmoen');
      expect(results[0].description, 'Bruk i Hemnes kommune');
      expect(results[0].position.longitude, 14.26984);
      expect(results[0].position.latitude, 65.90951);
    });
  });
}