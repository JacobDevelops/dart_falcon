class Label {
  final String value;
  const Label(this.value);
}

String render(String value) => value;

void example(Label label, String value) {
  print('value: ${label.value}');
  print('${render(value)}');
  print(r'$label.value');
  print(r'$render(value)');
  print('\$label.value');
  print('\$render(value)');
  print('$label.');
  print('$label...');
  print('$value.42');
  print('$label .value');
  print('$render (value)');
  print('${label}.value');
  print('${render}(value)');
  print('$value');
  print('literal label');
  print('''literal \$label.value''');
  print('$label' '.value');
  print(r'$label.value' '$value');
  print("${'$label'}.value");
  print('${{"closing": "}"}}.value');
  print('$label\u002evalue');
  print('$render\u0028value)');
  final String name = 'avatar';
  final String domain = 'example';
  final String methodName = 'render';
  print('assets/$name.png');
  print('user@$domain.com');
  print('logs $methodName()');
  print(r'value');
  print('\u0076alue');
  print('value' ' suffix');
  print('$label.unknown');
  print('$value.png');
  print('''
${label.value}.length
''');
}

void shadowing(String title, String? optional, dynamic unknown) {
  {
    final int title = 42;
    print('title');
  }
  print('optional');
  print('unknown');
  print('notInScope');
}

class Key {
  final String value;
  const Key(this.value);
}

class Preferences {
  void setString(String key, String value) {}
}

void named({required String key}) {}
void consume(Object first, Object second) {}

void keys(Preferences prefs, Map<String, String> json, String key) {
  final String email = 'address';
  final String token = 'secret';
  final String name = 'person';
  final fields = {'email': email};
  prefs.setString('token', token);
  print(json['name']);
  print(key == 'name');
  switch (key) {
    case 'name':
      break;
  }
  print(Key('email'));
  print(const Key('email'));
  named(key: 'email');
  consume('email', email.length);
  consume('email', '$email');
  final String assignment = 'name';
  print(fields);
  print(assignment);
  @Key('email')
  void annotated() {}
}

String returnName(String name) => 'name';

class Cache {
  String? get(String key) => null;
  void remove(String key) {}
}
void storageKeys(Cache prefs, String token, String id) {
  prefs.remove('token');
  prefs.get('id');
}
