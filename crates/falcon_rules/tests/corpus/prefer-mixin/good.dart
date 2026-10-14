import 'dart:collection';

mixin Logging {
  void log(String message) {}
}

mixin class Tracing {
  void trace() {}
}

class Service with Logging {}

class Worker extends Object with Logging, Tracing {}

class Alias = Object with Tracing;

enum Level with Logging { low, high }

// SDK mixins are fine.
class Numbers with ListMixin<int> {
  @override
  int length = 0;

  @override
  int operator [](int index) => index;

  @override
  void operator []=(int index, int value) {}
}

// Unresolved types are left alone.
class Remote with SomeUnknownMixin {}

// Extending or implementing a class is not mixing it in.
class Base {}

class Derived extends Base {}

class Impl implements Base {}
