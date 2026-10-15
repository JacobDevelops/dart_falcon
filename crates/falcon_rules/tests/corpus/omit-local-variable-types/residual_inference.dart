class Entry<T extends Object> {}

typedef IntCallback = int Function(int value);

class A {}

class B extends A {}

extension OnA on A {
  A get pick => A();
}

extension OnB on B {
  B get pick => B();
}

class Holder {
  int size = 0;
  Holder? next;
  Holder get self => this;
  int count() => 1;
}

abstract class Base {
  int measure(String text);
}

class Derived implements Base {
  measure(text) => 0;
}

void rawGenerics(Iterable<Entry> entries, Entry entry) {
  for (final Entry e in entries) {} /* expect: omit-local-variable-types */
  final Entry copy = entry; /* expect: omit-local-variable-types */
  print(copy);
}

void functionJoins(IntCallback? first, IntCallback? second) {
  final IntCallback? callback = first ?? second; /* expect: omit-local-variable-types */
  print(callback);
}

void specificExtension(B b) {
  final B picked = b.pick; /* expect: omit-local-variable-types */
  print(picked);
}

void nullShorting(Holder? holder) {
  final int? length = holder?.size; /* expect: omit-local-variable-types */
  final int? chained = holder?.self.count(); /* expect: omit-local-variable-types */
  final int? asserted = holder?.next!.size; /* expect: omit-local-variable-types */
  print([length, chained, asserted]);
}

void overrideInference(Derived derived) {
  final int measured = derived.measure(''); /* expect: omit-local-variable-types */
  print(measured);
}

void promotedTypeParameter<T>(T value, Iterable<T> values) {
  if (value is Iterable<T>) {
    final Iterable<T> promoted = value;
    print(promoted);
  }
}
