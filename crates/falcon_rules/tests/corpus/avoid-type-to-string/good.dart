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
