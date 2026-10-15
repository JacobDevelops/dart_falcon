int? _i; /* expect: use-late-for-private-fields-and-variables */

String? _name; /* expect: use-late-for-private-fields-and-variables */

void setUp() {
  _i = 1;
  _name = 'falcon';
}

void use() {
  print(_i!);
  print(_name!.length);
}

class Cache {
  List<int>? _items; /* expect: use-late-for-private-fields-and-variables */
  static String? _label; /* expect: use-late-for-private-fields-and-variables */
  Object? _value = Object(); /* expect: use-late-for-private-fields-and-variables */

  void init() {
    _items = [1, 2, 3];
    _label = 'cache';
  }

  int get first => _items!.first;
  String get label => _label!;
  Object get value => _value!;
}

class Counter {
  int? _count; /* expect: use-late-for-private-fields-and-variables */

  void reset() {
    this._count = 0;
  }

  int next() => _count! + 1;
}

// Each class's field is its own declaration: a plain read of the other
// class's same-named field does not disqualify this one.
class NullAsserted {
  int? _count; /* expect: use-late-for-private-fields-and-variables */

  int next() => _count! + 1;
}

// A read inside a class with a const constructor is not visited.
class ConstReader {
  const ConstReader();
  int? read() => _i;
}

// Writes through a type name keep the field lateable.
class Qualified {
  static int? _shared; /* expect: use-late-for-private-fields-and-variables */

  static int read() => _shared!;
}

void initShared() {
  Qualified._shared = 1;
}

extension _Helpers on String {
  static int? cached; /* expect: use-late-for-private-fields-and-variables */

  int get size {
    cached = 3;
    return cached!;
  }
}
