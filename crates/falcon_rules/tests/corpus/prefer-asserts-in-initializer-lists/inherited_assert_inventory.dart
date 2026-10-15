import 'dart:collection';

const limit = 3;
bool valid(int value) => value > 0;

abstract class InheritedList extends ListBase<int> {
  InheritedList(int value) {
    assert(limit > 0); /* expect: prefer-asserts-in-initializer-lists */
    assert(valid(value)); /* expect: prefer-asserts-in-initializer-lists */
    assert(isNotEmpty);
  }
}

mixin LocalMixin {
  int get count => 1;
}

abstract class MixedList extends ListBase<int> with LocalMixin {
  MixedList(int value) {
    assert(valid(value)); /* expect: prefer-asserts-in-initializer-lists */
    assert(count > 0);
  }
}
