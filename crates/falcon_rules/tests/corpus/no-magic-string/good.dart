import 'dart:core';
export 'dart:core';

const title = 'Title';
class Model { final label = 'Label'; }
void f(dynamic value, [String fallback = 'Fallback']) {
  final message = 'message';
  print('log');
  debugPrint('debug');
  assert(value != null, 'required');
  RegExp('pattern');
  Uri.https('example.com');
  StateError('error');
  ArgumentError.value(value, 'count', 'must be positive');
  RangeError.range(value, 0, 9, 'index');
  value.split(',');
  const map = {'key': 'value'};
  value['key'];
  consume('');
  value..['cascade-key'] = 1;
  value..['cascade-read'].toString();
  consume({'serialized-key': value});
  value.contains('direct');
  value.contains(('parenthesized'));
  print(message: 'named');
  final parenthesized = ('initializer');
  final interpolated = '${value}';
}

enum Env {
  dev('https://dev.example.com');
  const Env(this.url);
  final String url;
}

void patterns(dynamic value) {
  if (value case 'ready') {}
  switch (value) {
    case 'a':
      break;
    default:
      break;
  }
  final matched = switch (value) { 'b' => true, _ => false };
  if (value case {'user': var name}) {}
}

class Marker { const Marker(String label); }
@Marker('annotation')
void annotated() {}
