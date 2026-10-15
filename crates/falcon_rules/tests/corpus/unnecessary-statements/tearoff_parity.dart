// Source-defined contracts preserve analyzer tear-off parity without SDK indexes.
class C {}
class Clock {
  Clock.now();
}
class Values<T> {
  Values.filled();
  void map<U>() {}
}
void f(Values<int> l) {
  C.new; /* expect: unnecessary-statements */
  Clock.now; /* expect: unnecessary-statements */
  l.map<int>; /* expect: unnecessary-statements */
  Values<int>.filled; /* expect: unnecessary-statements */
}
