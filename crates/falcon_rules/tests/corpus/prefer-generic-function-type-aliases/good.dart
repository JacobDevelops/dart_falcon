// Modern generic function type syntax, plus non-function aliases.
typedef Compare = int Function(int a, int b);
typedef Callback = void Function(String msg);
typedef Predicate<T> = bool Function(T value);
typedef IntList = List<int>;
typedef Json = Map<String, dynamic>;
typedef VoidFn = void Function();
