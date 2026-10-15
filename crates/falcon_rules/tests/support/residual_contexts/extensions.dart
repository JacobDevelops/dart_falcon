class Item {}
class Pointer<T> {}
class Allocator {}
extension Allocation on Allocator {
  Pointer<T> call<T>() => Pointer<T>();
}
extension Reference<T> on Pointer<T> {
  T get ref => throw 0;
}
extension Bytes on Pointer<int> {
  List<int> asTypedList(int length) => [];
  int operator [](int index) => index;
}
