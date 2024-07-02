import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:map_app/data/datastore/sqlite/sqlite_marker_datastore.dart';

import 'indexeddb/indexdb.dart';
import 'marker_data_store.dart';

class MarkerDataStoreFactory {
  static MarkerDataStore? _db;

  static Future<MarkerDataStore> init() async {
    final MarkerDataStore dataStore;
    if (kIsWeb) {
      dataStore = ShimDBMarkerDataStore();
    } else {
      dataStore = SQLiteMarkerDataStore();
    }
    await dataStore.init();
    _db = dataStore;
    return dataStore;
  }

  static MarkerDataStore getDataStore(){
    if(_db == null){
      throw Error();
    }
    return _db!;
  }
}