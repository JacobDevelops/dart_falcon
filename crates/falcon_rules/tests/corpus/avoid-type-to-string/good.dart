mixin M {
  @override
  String toString() => '';
}

class Type2 with M implements Type {
  String get x => toString();
}

class Plain {
  String describe() => toString();
}

class Named {
  @override
  String toString() => 'Named';
}

void compareTypes(Object value) {
  if (value.runtimeType == int) {
    print('int');
  }
}

void otherToString(int number, Plain plain) {
  print(number.toString());
  print(plain.toString());
  print(Named().toString());
}

void interpolation(Type type) {
  print('$type');
}

void dynamicReceiver(dynamic value) {
  print(value.toString());
}

// `dart:core` `Type` redeclares `toString`, so `Type` receivers (including
// every `runtimeType` read) never resolve to `Object.toString`.
var fromRuntimeType = 7.runtimeType.toString();

void tearOff(void Function(String Function()) foo) {
  foo(7.runtimeType.toString);
}

extension E on Type {
  String describe() => toString();
}

class Widget {
  String label() => runtimeType.toString();
}

void typeParameter(Type type) {
  print(type.toString());
}
