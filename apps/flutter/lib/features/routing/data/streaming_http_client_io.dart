import 'package:http/http.dart' as http;

/// Mobile/desktop: the default client (IOClient) already delivers the
/// response body incrementally.
http.Client makeStreamingClient() => http.Client();
