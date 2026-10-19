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
