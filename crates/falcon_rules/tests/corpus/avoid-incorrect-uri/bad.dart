import 'dart:core';
import 'dart:core' as core;

void f(int count) {
  Uri(queryParameters: {'count': 1}); /* expect: avoid-incorrect-uri */
  Uri.http('example.com', '/', {'flag': false}); /* expect: avoid-incorrect-uri */
  Uri.https('example.com', '/', {'count': 1.5}); /* expect: avoid-incorrect-uri */
  Uri(path: '', pathSegments: ['']); /* expect: avoid-incorrect-uri */
  new Uri(queryParameters: {'count': count}); /* expect: avoid-incorrect-uri */
  core.Uri(queryParameters: {'count': 3}); /* expect: avoid-incorrect-uri */
  core.Uri.https('example.com', '/', {'values': ['valid', 2]}); /* expect: avoid-incorrect-uri */
  Uri(queryParameters: {'values': [null]}); /* expect: avoid-incorrect-uri */
  Uri(query: 'key=value', queryParameters: {'key': 'value'}); /* expect: avoid-incorrect-uri */
  core.Uri(query: '', queryParameters: <String, String>{}); /* expect: avoid-incorrect-uri */
}
