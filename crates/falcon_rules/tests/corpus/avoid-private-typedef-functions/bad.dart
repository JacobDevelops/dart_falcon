typedef _Unused = void Function(int); /* expect: avoid-private-typedef-functions */

typedef _UsedOnce = int Function(String); /* expect: avoid-private-typedef-functions */

typedef void _Legacy(int x); /* expect: avoid-private-typedef-functions */

typedef _LegacyGeneric<T> = void Function(T); // generic: not reported

typedef T _LegacyWithTypeParams<T>(T value); /* expect: avoid-private-typedef-functions */

typedef _Nullable = bool Function()?; /* expect: avoid-private-typedef-functions */

void register(_UsedOnce callback) {}

_Nullable? holder;
