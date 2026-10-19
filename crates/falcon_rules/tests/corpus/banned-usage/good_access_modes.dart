class WriteBase { int get writable => 0; }
class WriteChild extends WriteBase { set writable(int value) {} }
class ReadBase { int? get readable => 0; }
void access(WriteChild child, ReadChild reader) {
  print(child.writable);
  reader.readable = 1;
  reader..readable = 1;
}
class FieldClient {
  int flag = 0;
  int local() { final flag = 1; return flag; }
  int parameter(int flag) => flag;
  void closure() { void inner(int flag) => print(flag); inner(1); }
}
class ReadChild extends ReadBase {
  set readable(int? value) {}
  void write() { readable = 1; }
}
class BannedBox { int get size => 0; }
extension AllowedSized on BannedBox {
  int get size => 1;
  int twice() => size * 2;
}
class Counter {
  static int count = 0;
  int parameter(int count) => count;
  int local() { final count = 1; return count; }
}
