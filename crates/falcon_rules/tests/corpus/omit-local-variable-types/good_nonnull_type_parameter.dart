void nullableBound<T>(T? p) {
  final T y = p!;
  T Function() fn = () => p!;
  print([y, fn]);
}

void explicitNullableBound<T extends Object?>(T? p) {
  final T y = p!;
  T Function() fn = () => p!;
  print([y, fn]);
}

void promotedNullableBound<T>(T? p) {
  if (p != null) {
    final T y = p;
    print(y);
  }
}
