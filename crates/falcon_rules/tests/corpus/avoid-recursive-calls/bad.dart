int forever() => forever(); /* expect: avoid-recursive-calls */
int recursive(int n) { return recursive(n - 1); } /* expect: avoid-recursive-calls */
void again() { again(); } /* expect: avoid-recursive-calls */
class Broken {
  int run() => this.run(); /* expect: avoid-recursive-calls */
  static int staticRun() => staticRun(); /* expect: avoid-recursive-calls */
}
void outer() {
  int local() => local(); /* expect: avoid-recursive-calls */
  print(local);
}
int bounded(int n) => n <= 0 ? 0 : bounded(n - 1); /* expect: avoid-recursive-calls */
int guarded(int n) { if (n <= 0) return 0; return guarded(n - 1); } /* expect: avoid-recursive-calls */
Iterable<int> sequence() sync* { yield* sequence(); } /* expect: avoid-recursive-calls */
Future<void> asyncAgain() async => asyncAgain(); /* expect: avoid-recursive-calls */
