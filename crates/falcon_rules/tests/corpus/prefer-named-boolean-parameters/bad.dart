void _configure(bool verbose) {} /* expect: prefer-named-boolean-parameters */
void _optional([bool verbose = false]) {} /* expect: prefer-named-boolean-parameters */
void _nullable(bool? enabled) {} /* expect: prefer-named-boolean-parameters */
void _multiple(int count, bool verbose, bool dryRun) {} /* expect: prefer-named-boolean-parameters */

class Settings {
  Settings._internal(bool enabled); /* expect: prefer-named-boolean-parameters */
  Settings._optional([bool enabled = false]); /* expect: prefer-named-boolean-parameters */
  factory Settings._factory(bool enabled) => Settings._internal(enabled); /* expect: prefer-named-boolean-parameters */
  void _method(bool enabled) {} /* expect: prefer-named-boolean-parameters */
  static void _staticMethod(bool? enabled) {} /* expect: prefer-named-boolean-parameters */
  final callback = (bool enabled) {}; /* expect: prefer-named-boolean-parameters */
}

class Primary._hidden(bool enabled); /* expect: prefer-named-boolean-parameters */

final callback = (bool enabled) {}; /* expect: prefer-named-boolean-parameters */
final nullableCallback = (bool? enabled) => enabled; /* expect: prefer-named-boolean-parameters */
final optionalCallback = ([bool enabled = false]) {}; /* expect: prefer-named-boolean-parameters */

void outer() {
  void _local(bool enabled) {} /* expect: prefer-named-boolean-parameters */
  final nested = (int count, bool enabled) {}; /* expect: prefer-named-boolean-parameters */
  void _generic<bool>(bool enabled) {}
  void _sibling(bool enabled) {} /* expect: prefer-named-boolean-parameters */
  final generic = <bool>(bool enabled) {};
  final sibling = (bool enabled) {}; /* expect: prefer-named-boolean-parameters */
}

abstract class Contract {
  void _fixed(bool enabled); /* expect: prefer-named-boolean-parameters */
}
class Annotated extends Contract {
  @override
  void _fixed(bool enabled) {}
}
class Unannotated extends Contract {
  void _fixed(bool enabled) {}
  void _independent(bool enabled) {} /* expect: prefer-named-boolean-parameters */
}
class Generic<bool> {
  void _shadowed(bool enabled) {}
}
class AfterGeneric {
  void _unshadowed(bool enabled) {} /* expect: prefer-named-boolean-parameters */
}
