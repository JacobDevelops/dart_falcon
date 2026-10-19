class WriteBase { int get writable => 0; }
class WriteChild extends WriteBase { set writable(int value) {} }
class ReadBase { int? get readable => 0; }
class ReadChild extends ReadBase { set readable(int? value) {} }
class DualBase { int get both => 0; }
class DualChild extends DualBase { set both(int value) {} }
class ExtensionTarget {}
extension WriteExtension on ExtensionTarget { set value(int value) {} }
void writes(WriteChild child, ReadChild reader, DualChild dual, ExtensionTarget extensionTarget) {
  child.writable = 1; /* expect: banned-usage */
  child.writable += 1; /* expect: banned-usage */
  child.writable++; /* expect: banned-usage */
  ++child.writable; /* expect: banned-usage */
  reader.readable += 1; /* expect: banned-usage */
  reader.readable ??= 1; /* expect: banned-usage */
  dual.both = 1; /* expect: banned-usage, msg: "Avoid this setter." */
  dual.both += 1; /* expect: banned-usage, msg: "Avoid this setter." */
  print(dual.both); /* expect: banned-usage, msg: "Avoid this getter." */
  child..writable = 1; /* expect: banned-usage */
  child..writable += 1; /* expect: banned-usage */
  reader..readable ??= 1; /* expect: banned-usage */
  extensionTarget.value = 1; /* expect: banned-usage */
  WriteChild? nullable = child;
  nullable?.writable = 1; /* expect: banned-usage */
}
class FieldClient {
  int flag = 0;
  int read() => flag; /* expect: banned-usage */
  void write() { flag = 1; } /* expect: banned-usage */
  void increment() { ++flag; } /* expect: banned-usage */
  void decrement() { --flag; } /* expect: banned-usage */
}
class UnqualifiedWriteChild extends WriteChild {
  void write() { writable = 1; } /* expect: banned-usage */
}
class Box { int get size => 0; }
extension Sized on Box {
  int get size => 1;
  int twice() => size * 2; /* expect: banned-usage */
}
extension FirstExtension on Box {
  int get g => 0;
  int read() => g; /* expect: banned-usage */
}
extension SecondExtension on Box { int get g => 1; }
class Counter {
  static int count = 0;
  void bump() { count++; } /* expect: banned-usage */
}
class MultiMatch { int value = 0; }
void multipleMatches(MultiMatch object) {
  print(object.value); /* expect: banned-usage, msg: "Exact member ban." */ /* expect: banned-usage, msg: "Wildcard member ban." */
}
