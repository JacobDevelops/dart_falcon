// Only static methods: move them to the top level.
class DateUtils { /* expect: avoid-classes-with-only-static-members */
  static DateTime mostRecent(List<DateTime> dates) =>
      dates.reduce((a, b) => a.isAfter(b) ? a : b);
}

// Static non-const fields count as instance-free state.
class _Favorites { /* expect: avoid-classes-with-only-static-members */
  static int mostFavorite = 0;
}

// A static getter induces a non-const field.
class Config { /* expect: avoid-classes-with-only-static-members */
  static String get name => 'config';
}

// A default constructor does not make the class a real type.
class Helpers { /* expect: avoid-classes-with-only-static-members */
  Helpers();
  static void run() {}
  static const int retries = 3;
}

// `extends Object` adds no members.
abstract class Registry extends Object { /* expect: avoid-classes-with-only-static-members */
  static final List<String> names = [];
  static void register(String name) => names.add(name);
}

// Static setter plus static method.
class Counter { /* expect: avoid-classes-with-only-static-members */
  static int _count = 0;
  static set count(int value) => _count = value;
  static void reset() => _count = 0;
}

// Unlike members inherited from a class, mixin members do not exempt this lint.
mixin Greets {
  void greet() {}
}
class Greeter with Greets { /* expect: avoid-classes-with-only-static-members */
  static void helper() {}
}

// Empty and static-only parents do not contribute instance members.
class EmptyParent {}

class ExtendsEmpty extends EmptyParent { /* expect: avoid-classes-with-only-static-members */
  static int value = 1;
}

class ImplementsEmpty implements EmptyParent { /* expect: avoid-classes-with-only-static-members */
  static int value = 1;
}

class StaticParent { /* expect: avoid-classes-with-only-static-members */
  static int value = 1;
}

class ExtendsStatic extends StaticParent { /* expect: avoid-classes-with-only-static-members */
  static void helper() {}
}

mixin class StaticMixinClass { /* expect: avoid-classes-with-only-static-members */
  static int value = 1;
}

// Only the merged interface's owners count, not overridden class declarations.
class MethodParent {
  void run() {}
}
mixin ReplacesMethod on MethodParent {
  void run() {}
}
class ReplacedMethod extends MethodParent with ReplacesMethod { /* expect: avoid-classes-with-only-static-members */
  static void helper() {}
}

class AccessorParent {
  int get value => 0;
  set value(int value) {}
}
mixin ReplacesAccessors {
  int get value => 1;
  set value(int value) {}
}
class ReplacedAccessors extends AccessorParent with ReplacesAccessors { /* expect: avoid-classes-with-only-static-members */
  static void helper() {}
}
