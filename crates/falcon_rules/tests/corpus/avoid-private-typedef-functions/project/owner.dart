part 'part.dart';

typedef _UsedOnce = void Function(); /* expect: avoid-private-typedef-functions */
typedef _Reused = void Function();
