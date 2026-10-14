class Base {
  void print(Object? value) {}
}

class Derived extends Base {
  void inherited() {
    print('SDK print takes precedence over inherited members'); /* expect: avoid-print */
  }
}

class Example {
  final field = () {
    print('field'); /* expect: avoid-print */
  };
  Example() : value = (() {
    print('initializer'); /* expect: avoid-print */
  });
  final value;
  int get count {
    print('getter'); /* expect: avoid-print */
    return 0;
  }
  Example operator +(Example other) {
    print('operator'); /* expect: avoid-print */
    return this;
  }
}

void scopes(Object value) {
  if (value case final void Function(Object?) print) { print('pattern'); }
  print('after pattern'); /* expect: avoid-print */
  final callbacks = [for (final print in <void Function(Object?)>[(value) {}]) () { print('loop'); }];
  print('after collection loop'); /* expect: avoid-print */
  final closure = (void Function(Object?) print) { print('parameter'); };
}
