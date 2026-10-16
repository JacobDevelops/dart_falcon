class Registry {
  T call<T>({Object? param1, Object? param2}) => throw StateError('unused');
}

void check(Registry registry) {
  registry<int>(param1: null); /* expect: avoid-redundant-argument-values */
  registry<int>(param1: 'value');
  registry.call<int>(param2: null); /* expect: avoid-redundant-argument-values */
}
