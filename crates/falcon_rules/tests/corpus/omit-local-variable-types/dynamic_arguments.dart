class DynamicBox<T> { DynamicBox(); }
class DynamicGeneric<T> { T get value => throw ''; }
void dynamicArguments(dynamic value, List<dynamic> values,
    DynamicGeneric<dynamic> generic, dynamic Function() callback) {
  dynamic d = value;
  final List<dynamic> list = []; /* expect: omit-local-variable-types */
  final List<dynamic> copy = values; /* expect: omit-local-variable-types */
  final DynamicGeneric<dynamic> g = generic; /* expect: omit-local-variable-types */
  final dynamic Function() cb = callback; /* expect: omit-local-variable-types */
  final DynamicBox<dynamic> box = DynamicBox(); /* expect: omit-local-variable-types */
}
