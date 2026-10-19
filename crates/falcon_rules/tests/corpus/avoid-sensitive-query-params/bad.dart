final first = 'https://example.com/?password=secret'; /* expect: avoid-sensitive-query-params */
final second = 'http://example.com/path?page=1&access_token=value'; /* expect: avoid-sensitive-query-params */
final third = Uri(queryParameters: {'api_key': 'secret'}); /* expect: avoid-sensitive-query-params */
final fourth = Uri.https('example.com', '/path', {'client_secret': 'secret'}); /* expect: avoid-sensitive-query-params */
final fifth = new Uri(queryParameters: {'refresh_token': 'secret'}); /* expect: avoid-sensitive-query-params */
final sixth = Uri.http('example.com', '/', {'PASSWORD': 'secret'}); /* expect: avoid-sensitive-query-params */
final seventh = 'https://example.com/?token=$token'; /* expect: avoid-sensitive-query-params */
String token = 'secret';

void interpolationPunctuation(String host, String path, String key) {
  final hostQuery = 'https://${host.replaceAll('#', '')}/v1?api_key=$key'; /* expect: avoid-sensitive-query-params */
  final pathQuery = 'https://example.com/${path.replaceAll('#', '')}?password=$key'; /* expect: avoid-sensitive-query-params */
  final laterQuery = 'https://example.com/?page=${path.replaceAll('', '#')}&token=$key'; /* expect: avoid-sensitive-query-params */
}

void literalQueries(String token) {
  final direct = Uri(scheme: 'https', host: 'example.com', query: 'access_token=$token'); /* expect: avoid-sensitive-query-params */
  final explicitNew = new Uri(scheme: 'https', host: 'example.com', query: ('page=1&api_key=$token')); /* expect: avoid-sensitive-query-params */
  final interpolation = Uri(query: 'page=${token.replaceAll('', '#')}&password=$token'); /* expect: avoid-sensitive-query-params */
  final hashInValue = Uri(query: 'next=/home#top&access_token=$token'); /* expect: avoid-sensitive-query-params */
  final adjacentKey = Uri(query: 'page=1&' 'access_token=$token'); /* expect: avoid-sensitive-query-params */
}

final encodedUrl = 'https://example.com/?%61ccess_token=value'; /* expect: avoid-sensitive-query-params */
final encodedQuery = Uri(query: '%61ccess_token=value'); /* expect: avoid-sensitive-query-params */
final encodedSeparator = 'https://example.com/?access%5ftoken=value'; /* expect: avoid-sensitive-query-params */
final encodedUppercase = new Uri(query: '%50ASSWORD=value'); /* expect: avoid-sensitive-query-params */
