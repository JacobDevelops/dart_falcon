class Uri {
  Uri({Map<String, String>? queryParameters, String? query});
  Uri.https(String host, String path, Map<String, String> queryParameters);
}
final custom = Uri(queryParameters: {'token': 'not a network URL'});
final customNew = new Uri(queryParameters: {'password': 'not a network URL'});
final customHttps = Uri.https('example.com', '/', {'api_key': 'not a network URL'});
final customQuery = Uri(query: 'access_token=not-a-network-url');
final customNewQuery = new Uri(query: 'api_key=not-a-network-url');
