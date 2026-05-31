import 'package:http/http.dart' as http;

import 'streaming_http_client_io.dart'
    if (dart.library.js_interop) 'streaming_http_client_web.dart' as impl;

/// A [http.Client] that reads responses **incrementally** — required for
/// the routing SSE stream to deliver progress events as they arrive.
///
/// On mobile/desktop the default IO client already streams. On web, dio's
/// XHR adapter buffers the whole response, so we use the Fetch API
/// (`fetch_client`) which exposes the response body as a readable stream.
http.Client makeStreamingClient() => impl.makeStreamingClient();
