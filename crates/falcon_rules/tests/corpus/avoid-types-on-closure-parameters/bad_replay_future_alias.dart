typedef Handler = void Function(int);
typedef FH = Future<Handler>;
FH make() async {
  return (int x) {}; /* expect: avoid-types-on-closure-parameters */
}
