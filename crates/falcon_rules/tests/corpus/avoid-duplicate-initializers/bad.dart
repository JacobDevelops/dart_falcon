void fields(dynamic param) {
  final first = param.value.isNotEmpty;
  final second = param.value.isNotEmpty; /* expect: avoid-duplicate-initializers */
}
void identifiers(int input) {
  final first = input;
  final second = input; /* expect: avoid-duplicate-initializers */
}
void operators(int input) {
  final first = input + 1;
  final second = input + 1; /* expect: avoid-duplicate-initializers */
}
void constants() {
  const first = 'same';
  const second = 'same'; /* expect: avoid-duplicate-initializers */
}
class A {
  final first = 42;
  final second = 42; /* expect: avoid-duplicate-initializers */
}
class B {
  static const first = true;
  static const second = true; /* expect: avoid-duplicate-initializers */
}
class LazyConstants {
  static final first = 1 + 2;
  static final second = 1 + 2; /* expect: avoid-duplicate-initializers */
}
class InstanceSnapshots {
  final first = current;
  final second = current; /* expect: avoid-duplicate-initializers */
}
int current = 0;
class MixedSnapshots {
  final first = 1;
  static const firstConst = 2;
  static final snapshot = current;
  final second = 1; /* expect: avoid-duplicate-initializers */
  static const secondConst = 2; /* expect: avoid-duplicate-initializers */
}
