class Immutable {
  const Immutable();
}

@Immutable()
class Mutable {
  @override
  bool operator ==(Object other) => other is Mutable; /* expect: avoid-equals-and-hash-code-on-mutable-classes */

  @override
  int get hashCode => 0; /* expect: avoid-equals-and-hash-code-on-mutable-classes */
}
