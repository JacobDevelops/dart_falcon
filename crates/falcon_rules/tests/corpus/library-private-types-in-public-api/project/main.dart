part 'part.dart';

class _P {}
class _A {
  _A(_P o);
}
class B extends _A {
  B(super.o); /* expect: library-private-types-in-public-api */
}
