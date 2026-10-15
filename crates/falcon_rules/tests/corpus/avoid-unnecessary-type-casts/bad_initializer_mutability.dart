// Mutable explicit locals still promote; inferred final locals still infer non-null types.
void mutableAndInferredInitializers() {
  String? mutable = 'x';
  print(mutable as String); /* expect: avoid-unnecessary-type-casts */
  final inferred = 'x';
  print(inferred as String); /* expect: avoid-unnecessary-type-casts */
}

// A non-null const initializer promotes its nullable local to int.
void explicitlyTypedConst() {
  const int? count = 1;
  print(count as int); /* expect: avoid-unnecessary-type-casts */
}
