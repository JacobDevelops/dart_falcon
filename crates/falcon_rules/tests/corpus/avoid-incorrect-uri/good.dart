import 'dart:core';
import 'dart:core' as core;

void f(dynamic unknown, Iterable<String> values, String? path) {
  Uri(fragment: '', queryParameters: {'hello': 'world'});
  Uri.https('example.com', '/', {'hello': 'world'});
  Uri.http('example.com', '/', {'hello': ['world', 'again']});
  Uri(path: '');
  Uri(pathSegments: ['']);
  Uri(path: null, pathSegments: ['']);
  Uri(path: path, pathSegments: ['']);
  Uri(queryParameters: {'empty': null});
  Uri(queryParameters: {'empty': <String>[]});
  Uri(queryParameters: {'values': <String>{'first', 'second'}});
  Uri(queryParameters: {'values': values});
  Uri(queryParameters: {'unknown': unknown});
  core.Uri(queryParameters: {'hello': 'world'});
  Uri(query: 'key=value');
  Uri(queryParameters: {'key': 'value'});
  Uri(query: null, queryParameters: {'key': 'value'});
  Uri(query: 'key=value', queryParameters: null);
  Uri(query: path, queryParameters: {'key': 'value'});
  core.Uri(query: '', queryParameters: <String, String>{});
  Uri(query: (r''), queryParameters: {'key': 'value'});
}

void shadowed(dynamic Uri) {
  Uri(queryParameters: {'count': 1});
  Uri.https('', '', {'count': 1});
}

class CustomUri {
  CustomUri({dynamic queryParameters});
}
void other() {
  CustomUri(queryParameters: {'count': 1});
}

void collections(bool includeCount, List<String> values) {
  Uri(queryParameters: {
    if (includeCount) 'count': '1' else 'other': null,
    for (final value in values) value: value,
    for (final value in values)
      if (includeCount) 'nested$value': <String>['valid'],
  });
}
