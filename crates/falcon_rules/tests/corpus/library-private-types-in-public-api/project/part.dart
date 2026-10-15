part of 'main.dart';

class R {
  R(this.p); /* expect: library-private-types-in-public-api */
  final _P p; /* expect: library-private-types-in-public-api */
}
