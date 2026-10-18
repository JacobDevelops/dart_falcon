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
