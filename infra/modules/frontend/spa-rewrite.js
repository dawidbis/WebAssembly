// CloudFront Function (viewer request) dla zachowania domyślnego (S3).
// Ścieżki bez rozszerzenia (trasy aplikacji) dostają index.html – fallback SPA.
// Działa tylko na zachowaniu domyślnym, więc /api/* i /ws* go nie widzą.
function handler(event) {
  var request = event.request;
  var last = request.uri.substring(request.uri.lastIndexOf('/') + 1);
  if (last.indexOf('.') === -1) {
    request.uri = '/index.html';
  }
  return request;
}
