class Uri {
  Uri({Map<String, String>? queryParameters});
  Uri.https(String host, String path, Map<String, String> queryParameters);
}
final custom = Uri(queryParameters: {'token': 'not a network URL'});
final customNew = new Uri(queryParameters: {'password': 'not a network URL'});
final customHttps = Uri.https('example.com', '/', {'api_key': 'not a network URL'});
