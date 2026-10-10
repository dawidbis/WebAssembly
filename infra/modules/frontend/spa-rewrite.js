// CloudFront Function (viewer request, runtime cloudfront-js-2.0) dla zachowania domyślnego (S3).
// Ścieżki bez rozszerzenia (trasy aplikacji) dostają index.html – fallback SPA.
// Działa tylko na zachowaniu domyślnym, więc /api/* i /ws* go nie widzą.
function handler(event) {
  const request = event.request;
  const last = request.uri.substring(request.uri.lastIndexOf('/') + 1);
  if (!last.includes('.')) {
    request.uri = '/index.html';
  }
  return request;
}
