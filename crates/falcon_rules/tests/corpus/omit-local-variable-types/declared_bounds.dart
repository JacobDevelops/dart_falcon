class Route<T> {}

class Routes {
  static Route<T>? of<T extends Object?>(int context) => null;
}

Route<T>? anyRoute<T>() => null;

class Bounded<T extends Object?> {
  Bounded();
}

void declaredBounds(int context) {
  // A `dynamic` context leaves `T extends Object?` at its bound, `Object?`.
  final Route<dynamic>? route = Routes.of(context);
  final Bounded<dynamic> bounded = Bounded();
  final Route<Object?>? exact = Routes.of(context); /* expect: omit-local-variable-types */
  // An omitted bound adds no constraint, so the context instantiates `dynamic`.
  final Route<dynamic>? omitted = anyRoute(); /* expect: omit-local-variable-types */
  print([route, bounded, exact, omitted]);
}
