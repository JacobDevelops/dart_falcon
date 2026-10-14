class Point {
  final int x;

  Point(this.x);

  @override
  String toString() => '$runtimeType($x)'; /* expect: no-runtime-type-to-string */

  String describe() => runtimeType.toString(); /* expect: no-runtime-type-to-string */

  String explicit() => this.runtimeType.toString(); /* expect: no-runtime-type-to-string */

  String viaSuper() => super.runtimeType.toString(); /* expect: no-runtime-type-to-string */

  String braced() => 'Instance of ${this.runtimeType}'; /* expect: no-runtime-type-to-string */

  void log() {
    final callback = () => print('$runtimeType'); /* expect: no-runtime-type-to-string */
    callback();
  }
}

enum Color {
  red;

  String label() => '$runtimeType.$name'; /* expect: no-runtime-type-to-string */
}

extension on Point {
  String tag() => runtimeType.toString(); /* expect: no-runtime-type-to-string */
}
