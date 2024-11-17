class KartverketApiRequest{
  final String navn;

  KartverketApiRequest({required this.navn});

  factory KartverketApiRequest.of(String navn) {
    return KartverketApiRequest(navn: navn);
  }

  Uri uri(){
    return Uri.parse(
        'https://api.kartverket.no/stedsnavn/v1/navn?sok=${Uri.encodeComponent(navn)}*&fuzzy=true&treffPerSide=5');
  }
}

class KartverketApiResponse {
  final Metadata metadata;
  final List<PlaceName> navn;

  KartverketApiResponse({required this.metadata, required this.navn});

  factory KartverketApiResponse.fromJson(Map<String, dynamic> json) {
    return KartverketApiResponse(
      metadata: Metadata.fromJson(json['metadata']),
      navn: (json['navn'] as List).map((e) => PlaceName.fromJson(e)).toList(),
    );
  }
}

class Metadata {
  final int side;
  final String sokeStreng;
  final int totaltAntallTreff;
  final int treffPerSide;
  final int viserFra;
  final int viserTil;

  Metadata({
    required this.side,
    required this.sokeStreng,
    required this.totaltAntallTreff,
    required this.treffPerSide,
    required this.viserFra,
    required this.viserTil,
  });

  factory Metadata.fromJson(Map<String, dynamic> json) {
    return Metadata(
      side: json['side'],
      sokeStreng: json['sokeStreng'],
      totaltAntallTreff: json['totaltAntallTreff'],
      treffPerSide: json['treffPerSide'],
      viserFra: json['viserFra'],
      viserTil: json['viserTil'],
    );
  }
}

class PlaceName {
  final List<County> fylker;
  final List<Municipality> kommuner;
  final String navneobjekttype;
  final String navnestatus;
  final RepresentasjonsPunkt representasjonspunkt;
  final String skrivemate;
  final String skrivematestatus;
  final String sprak;
  final int stedsnummer;
  final String stedstatus;

  PlaceName({
    required this.fylker,
    required this.kommuner,
    required this.navneobjekttype,
    required this.navnestatus,
    required this.representasjonspunkt,
    required this.skrivemate,
    required this.skrivematestatus,
    required this.sprak,
    required this.stedsnummer,
    required this.stedstatus,
  });

  factory PlaceName.fromJson(Map<String, dynamic> json) {
    return PlaceName(
      fylker: (json['fylker'] as List).map((e) => County.fromJson(e)).toList(),
      kommuner: (json['kommuner'] as List)
          .map((e) => Municipality.fromJson(e))
          .toList(),
      navneobjekttype: json['navneobjekttype'],
      navnestatus: json['navnestatus'],
      representasjonspunkt:
          RepresentasjonsPunkt.fromJson(json['representasjonspunkt']),
      skrivemate: json['skrivemåte'],
      skrivematestatus: json['skrivemåtestatus'],
      sprak: json['språk'],
      stedsnummer: json['stedsnummer'],
      stedstatus: json['stedstatus'],
    );
  }
}

class County {
  final String fylkesnavn;
  final String fylkesnummer;

  County({required this.fylkesnavn, required this.fylkesnummer});

  factory County.fromJson(Map<String, dynamic> json) {
    return County(
      fylkesnavn: json['fylkesnavn'],
      fylkesnummer: json['fylkesnummer'],
    );
  }
}

class Municipality {
  final String kommunenavn;
  final String kommunenummer;

  Municipality({required this.kommunenavn, required this.kommunenummer});

  factory Municipality.fromJson(Map<String, dynamic> json) {
    return Municipality(
      kommunenavn: json['kommunenavn'],
      kommunenummer: json['kommunenummer'],
    );
  }
}

class RepresentasjonsPunkt {
  final int koordsys;
  final double nord;
  final double ost;

  RepresentasjonsPunkt(
      {required this.koordsys, required this.nord, required this.ost});

  factory RepresentasjonsPunkt.fromJson(Map<String, dynamic> json) {
    return RepresentasjonsPunkt(
      koordsys: json['koordsys'],
      nord: json['nord'],
      ost: json['øst'],
    );
  }
}
