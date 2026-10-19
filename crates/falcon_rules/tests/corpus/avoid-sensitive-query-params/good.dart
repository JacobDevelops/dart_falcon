final first = 'https://example.com/password';
final second = 'https://example.com/?page=1#token=secret';
final third = Uri(queryParameters: {'page': 'token'});
final fourth = Uri.https('example.com', '/', {'search': 'password'});
final fifth = {'password': 'not a query'};
final sixth = 'The password=secret example is not a URL';
final seventh = 'https://example.com/?token_count=10';
final eighth = Uri.http('example.com', '/token', {'page': '1'});
final fragmentQuery = 'https://app.example.com/#/reset?token=abc';
void shadowed(dynamic Uri) {
  Uri(queryParameters: {'token': 'not a URL'});
  Uri.https('example.com', '/', {'password': 'not a URL'});
  Uri(query: 'token=not-a-network-url');
  Uri(query: '%61ccess_token=not-a-network-url');
}

void interpolationPunctuation(String host, String path, String value) {
  final fragment = 'https://${host.replaceAll('#', '')}/#reset?token=$value';
  final pathOnly = 'https://${host.replaceAll('#', '')}/password=$value';
  final safeQuery = 'https://$host/${path.replaceAll('#', '')}?page=$value';
  final valuePunctuation = 'https://example.com/?page=${value.replaceAll('', '&token=')}';
  final dynamicKey = 'https://example.com/?api_${value}=secret';
  final encodedDynamicKey = 'https://example.com/?%61ccess_${value}=secret';
}

void safeLiteralQueries(String value) {
  final safe = Uri(scheme: 'https', host: 'example.com', query: 'page=$value');
  final fragment = Uri(scheme: 'https', host: 'example.com', fragment: 'token=secret');
  final dynamicQuery = Uri(query: value);
  final dynamicKey = Uri(query: 'api_${value}=secret');
  final dynamicValue = Uri(query: 'page=${value.replaceAll('', '&token=')}');
  query(query: 'token=not-a-url');
  final adjacentValue = Uri(query: 'page=' 'token=not-a-query-key');
}

void query({String? query}) {}

void unrelatedQuery(dynamic client) {
  client.query(query: 'token=not-a-url');
}

final encodedSafeUrl = 'https://example.com/?%70age=1';
final encodedSafeQuery = Uri(query: '%70age=1');
final doubleEncodedUrl = 'https://example.com/?%2561ccess_token=value';
final doubleEncodedQuery = Uri(query: '%2561ccess_token=value');
final malformedEscape = 'https://example.com/?access_token%6=value';
final incompleteEscape = Uri(query: 'access_token%=value');
final invalidHexEscape = Uri(query: '%GGaccess_token=value');
final invalidUtf8Escape = Uri(query: '%FFaccess_token=value');
final encodedMapKey = Uri(queryParameters: {'%61ccess_token': 'value'});
final encodedHttpsMapKey = Uri.https('example.com', '/', {'%61ccess_token': 'value'});
final encodedFragment = 'https://example.com/#%61ccess_token=value';
