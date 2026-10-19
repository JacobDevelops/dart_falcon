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
}

void interpolationPunctuation(String host, String path, String value) {
  final fragment = 'https://${host.replaceAll('#', '')}/#reset?token=$value';
  final pathOnly = 'https://${host.replaceAll('#', '')}/password=$value';
  final safeQuery = 'https://$host/${path.replaceAll('#', '')}?page=$value';
  final valuePunctuation = 'https://example.com/?page=${value.replaceAll('', '&token=')}';
  final dynamicKey = 'https://example.com/?api_${value}=secret';
}
