import 'dart:convert';

import 'package:meta/meta.dart';

/// The entry point; see [DocumentedOnly] and [Service.describe].
void main() {
  final service = Service.create();
  service.run();
  service.counter += 1;
  print(service[0]);
  print(-service);
  print(TypeLiteralOnly);
  print(<TypeArgumentOnly>[]);
  Callback? callback;
  callback = () {};
  callback();
  print(1 as Wrapped);
  print(jsonEncode([Serializable()]));
  print(Palette.red.label);
  print(Box(1).unwrap());
  final point = Point(1, 2);
  if (point case Point(:final x)) {
    print(x + point.y);
  }
  print(Derived().inherited());
  print(Holder.instance);
  print(OutsideError().detail);
  topLevelSetter = 3;
  print(topLevelGetter);
  annotatedByConstant();
}

class DocumentedOnly {}

class Service {
  Service._();

  factory Service.create() = Service._;

  int counter = 0;

  void run() {
    _helper();
  }

  void _helper() {}

  static void describe() {}

  int operator [](int index) => index;

  Service operator -() => this;

  Service operator +(int other) => this;

  @visibleForTesting
  void inspect() {}

  void test_harnessMethod() {}

  void setUp() {}
}

class TypeLiteralOnly {}

class TypeArgumentOnly {}

typedef Callback = void Function();

extension type Wrapped(int value) {}

class Serializable {
  List<Object> toJson() => const ['value'];
}

enum Palette {
  red('red');

  const Palette(this.label);

  final String label;
}

class Box<T> {
  Box(this.value);

  final T value;

  T unwrap() => value;
}

class Point {
  Point(this.x, this.y);

  final int x;
  final int y;
}

class Parent {
  Parent();

  String inherited() => 'parent';
}

class Derived extends Parent {}

class Holder {
  static final Holder instance = Holder._();

  Holder._();
}

class OutsideError extends Error {
  String get detail => 'outside';
}

set topLevelSetter(int value) {}

int get topLevelGetter => 0;

const entry = pragma('vm:entry-point');

@entry
void annotatedByConstant() {}

@pragma('vm:entry-point')
void nativeCallback() {
  calledFromCallback();
}

void calledFromCallback() {}

@visibleForTesting
void testOnlyHook() {}
