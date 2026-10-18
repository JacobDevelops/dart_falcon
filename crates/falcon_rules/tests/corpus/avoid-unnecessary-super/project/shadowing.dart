import 'bindings.dart';

class ImportedBase {
  void action() {}
  int get unshadowed => 9;
}
final class ImportedChild extends ImportedBase {
  void run() => super.action();
  int read() => super.unshadowed; /* expect: avoid-unnecessary-super */
}
