/// A utility function that sanitizes a URL template to be used as a
/// safe, unique identifier for a tile provider.
String sanitizeProviderId(String urlTemplate) {
  return urlTemplate
      .replaceAll(RegExp(r'https?://'), '')
      .replaceAll(RegExp(r'[^a-zA-Z0-9.-]'), '_');
}