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
  value.split(',');
  const map = {'key': 'value'};
  value['key'];
  consume('');
  value..['cascade-key'] = 1;
  value..['cascade-read'].toString();
  consume({'serialized-key': value});
}

enum Env {
  dev('https://dev.example.com');
  const Env(this.url);
  final String url;
}
