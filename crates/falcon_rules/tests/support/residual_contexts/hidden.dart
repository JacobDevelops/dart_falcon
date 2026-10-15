import 'extensions.dart';
class Marker {}
extension HiddenBytes on Pointer<int> {
  String asTypedList(int length) => '';
}
extension HiddenReference<T> on Pointer<T> {
  String get ref => '';
}
