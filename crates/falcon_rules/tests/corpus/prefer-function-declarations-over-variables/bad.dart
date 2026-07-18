// Function literals bound to final/const locals should be declarations.
Future<void> g() async {}

void f() {
  final greet = () { print('hi'); }; /* expect: prefer-function-declarations-over-variables */
  final add = (int a, int b) => a + b; /* expect: prefer-function-declarations-over-variables */
  final square = (int x) => x * x; /* expect: prefer-function-declarations-over-variables */
  final log = (String m) { print(m); }; /* expect: prefer-function-declarations-over-variables */
  final wrap = () async { await g(); }; /* expect: prefer-function-declarations-over-variables */
  final noop = () {}; /* expect: prefer-function-declarations-over-variables */
  print(add(1, 2) + square(3) + greet.hashCode + log.hashCode + wrap.hashCode + noop.hashCode);
}
