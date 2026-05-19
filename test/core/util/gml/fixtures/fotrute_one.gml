<?xml version='1.0' encoding='UTF-8'?>
<wfs:FeatureCollection xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:schemaLocation="http://www.opengis.net/wfs/2.0 http://schemas.opengis.net/wfs/2.0/wfs.xsd http://www.opengis.net/gml/3.2 http://schemas.opengis.net/gml/3.2.1/gml.xsd http://skjema.geonorge.no/SOSI/produktspesifikasjon/TurOgFriluftsruter/20171210 https://wfs.geonorge.no/skwms1/wfs.turogfriluftsruter?SERVICE=WFS&amp;VERSION=2.0.0&amp;REQUEST=DescribeFeatureType&amp;OUTPUTFORMAT=text%2Fxml%3B+subtype%3Dgml%2F3.2.1&amp;TYPENAME=app:Fotrute&amp;NAMESPACES=xmlns(app,http%3A%2F%2Fskjema.geonorge.no%2FSOSI%2Fproduktspesifikasjon%2FTurOgFriluftsruter%2F20171210)" xmlns:wfs="http://www.opengis.net/wfs/2.0" timeStamp="2026-05-19T07:49:11Z" xmlns:gml="http://www.opengis.net/gml/3.2" numberMatched="unknown" numberReturned="0">
  <!--NOTE: numberReturned attribute should be 'unknown' as well, but this would not validate against the current version of the WFS 2.0 schema (change upcoming). See change request (CR 144): https://portal.opengeospatial.org/files?artifact_id=43925.-->
  <wfs:member>
    <app:Fotrute xmlns:app="http://skjema.geonorge.no/SOSI/produktspesifikasjon/TurOgFriluftsruter/20171210" gml:id="fotrute.100128">
      <app:identifikasjon>
        <app:Identifikasjon>
          <app:lokalId>e76cb78a-bf00-40d0-ba56-6efc13cafee3</app:lokalId>
          <app:navnerom>http://data.geonorge.no/TurruterNGIS/Turruter/so</app:navnerom>
          <app:versjonId>2026-02-05 15:55:26.217713000</app:versjonId>
        </app:Identifikasjon>
      </app:identifikasjon>
      <app:datafangstdato>1984-01-01</app:datafangstdato>
      <app:oppdateringsdato>2026-02-05T14:55:26</app:oppdateringsdato>
      <app:kvalitet>
        <app:Posisjonskvalitet>
          <app:målemetode>55</app:målemetode>
          <app:nøyaktighet>1500</app:nøyaktighet>
        </app:Posisjonskvalitet>
      </app:kvalitet>
      <app:opphav>N50-kartdata</app:opphav>
      <app:informasjon>Geometri hentet fra TraktorvegSti, 20170619</app:informasjon>
      <app:senterlinje>
        <!--Inlined geometry 'fotrute.100128_APP_SENTERLINJE'-->
        <gml:LineString gml:id="fotrute.100128_APP_SENTERLINJE" srsName="urn:ogc:def:crs:EPSG::4326">
          <gml:posList>61.673901 8.375147 61.673580 8.373662 61.673376 8.372400 61.673175 8.371535 61.672965 8.370727 61.672983 8.370325</gml:posList>
        </gml:LineString>
      </app:senterlinje>
      <app:merking>JA</app:merking>
      <app:ruteFølger>ST</app:ruteFølger>
      <app:underlagstype></app:underlagstype>
      <app:rutebredde></app:rutebredde>
      <app:trafikkbelastning></app:trafikkbelastning>
      <app:sesong></app:sesong>
      <app:fotruteInfo>
        <app:FotruteInfo>
          <app:rutenavn>Ukjent</app:rutenavn>
          <app:rutenummer>jot45</app:rutenummer>
          <app:vedlikeholdsansvarlig>DNT | DNT Oslo og omegn</app:vedlikeholdsansvarlig>
          <app:spesialFotrutetype></app:spesialFotrutetype>
          <app:gradering></app:gradering>
          <app:rutetype></app:rutetype>
          <app:rutebetydning></app:rutebetydning>
          <app:tilpasning></app:tilpasning>
        </app:FotruteInfo>
        <app:FotruteInfo>
          <app:rutenavn>Ukjent</app:rutenavn>
          <app:rutenummer>jot42</app:rutenummer>
          <app:vedlikeholdsansvarlig>DNT | DNT Oslo og omegn</app:vedlikeholdsansvarlig>
          <app:spesialFotrutetype></app:spesialFotrutetype>
          <app:gradering></app:gradering>
          <app:rutetype></app:rutetype>
          <app:rutebetydning></app:rutebetydning>
          <app:tilpasning></app:tilpasning>
        </app:FotruteInfo>
        <app:FotruteInfo>
          <app:rutenavn>Bøverdalen</app:rutenavn>
          <app:rutenummer>F_20170606</app:rutenummer>
          <app:vedlikeholdsansvarlig>Lom kommune</app:vedlikeholdsansvarlig>
          <app:spesialFotrutetype></app:spesialFotrutetype>
          <app:gradering></app:gradering>
          <app:rutetype></app:rutetype>
          <app:rutebetydning></app:rutebetydning>
          <app:tilpasning></app:tilpasning>
        </app:FotruteInfo>
      </app:fotruteInfo>
    </app:Fotrute>
  </wfs:member>
</wfs:FeatureCollection>