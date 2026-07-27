abstract class Base { void method(int value, [String? label]); }
abstract class Child extends Base { void method(int value, [String? label, bool? extra]); }

class Equality {
  bool operator ==(Object other) => false;
  dynamic noSuchMethod(Invocation invocation) => null;
}

abstract class StaticBase { static void method(int original) {} }
abstract class InstanceChild extends StaticBase { void method(int renamed) {} }
