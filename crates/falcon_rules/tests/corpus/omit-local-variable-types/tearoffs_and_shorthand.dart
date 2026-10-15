class Box<T> {
  Box(T value);
  Box.named(T value);
  T method(T value) => value;
  U convert<U>(U value) => value;
  static int staticMethod(int value) => value;
  static Box<int> get empty => Box(0);
}
enum Choice { first, second }
T identity<T>(T value) => value;
void use(Box<int> box) {
  Type simple = int; /* expect: omit-local-variable-types */
  Type bottom = Never; /* expect: omit-local-variable-types */
  Type generic = Box<int>; /* expect: omit-local-variable-types */
  Symbol symbol = #member; /* expect: omit-local-variable-types */
  int Function(int) function = identity<int>; /* expect: omit-local-variable-types */
  int Function(int) downward = identity; /* expect: omit-local-variable-types */
  Box<int> Function(int) constructor = Box<int>.new; /* expect: omit-local-variable-types */
  Box<int> Function(int) named = Box<int>.named; /* expect: omit-local-variable-types */
  int Function(int) method = box.method; /* expect: omit-local-variable-types */
  int Function(int) genericMethod = box.convert<int>; /* expect: omit-local-variable-types */
  int Function(int) staticMethod = Box.staticMethod; /* expect: omit-local-variable-types */
  Choice choice = .first; /* expect: omit-local-variable-types */
  Box<int> getter = .empty; /* expect: omit-local-variable-types */
  Box<int> instance = .new(1); /* expect: omit-local-variable-types */
  print([simple, bottom, generic, symbol, function, downward, constructor, named, method, genericMethod, staticMethod, choice, getter, instance]);
}
