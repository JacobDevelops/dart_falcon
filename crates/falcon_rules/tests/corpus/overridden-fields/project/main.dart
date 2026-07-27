import 'base.dart' show Base;
import 'duplicate.dart' as duplicate;

class Child extends Base {
  int value = 1; /* expect: overridden-fields */
  int abstractValue = 1;
  int computed = 1;
  int staticValue = 1;
  String flexible = '';
  int _private = 1;
}

class Unrelated extends duplicate.Base {
  int value = 1;
}
