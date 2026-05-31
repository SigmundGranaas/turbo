import 'package:fetch_client/fetch_client.dart';
import 'package:http/http.dart' as http;

/// Web: the Fetch API exposes the response body as a readable stream, so
/// SSE frames arrive incrementally (unlike the XHR-based default).
http.Client makeStreamingClient() => FetchClient(mode: RequestMode.cors);
