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

class ExplicitDespiteShadow {
  String describe() => this.runtimeType.toString(); /* expect: no-runtime-type-to-string */

  void unrelated(String runtimeType) {}
}

class ExemptionRestoration {
  ExemptionRestoration()
      : assert(runtimeType.toString().isNotEmpty) {
    print('$runtimeType'); /* expect: no-runtime-type-to-string */
  }

  void inspect() {
    try {
      print('$runtimeType'); /* expect: no-runtime-type-to-string */
    } catch (error) {
      print('$runtimeType');
    } finally {
      print('$runtimeType'); /* expect: no-runtime-type-to-string */
    }
    print('$runtimeType'); /* expect: no-runtime-type-to-string */
  }
}

class PrimaryInitializer(int value) {
  this : assert(runtimeType.toString().isNotEmpty); /* expect: no-runtime-type-to-string */
}

abstract class ExemptBeforeConcrete {
  String describe() => '$runtimeType';
}

class CheckedAfterAbstract {
  String describe() => '$runtimeType'; /* expect: no-runtime-type-to-string */
}
