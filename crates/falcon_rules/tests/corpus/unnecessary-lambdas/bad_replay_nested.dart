void consume(int value) {}
void f() {
  final callback = (int x) => ((int y) => consume(y))(x); /* expect: unnecessary-lambdas */ /* expect: unnecessary-lambdas */
  callback(1);
}
