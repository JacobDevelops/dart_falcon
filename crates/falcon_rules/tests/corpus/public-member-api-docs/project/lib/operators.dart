/// A unary operator declaration.
class UnaryBase {
  /// Negates this value.
  UnaryBase operator -() => this;
}

/// Inherits the unary operator.
class UnaryChild extends UnaryBase {
  @override
  UnaryChild operator -() => this;
}

/// Declares only binary subtraction.
class BinaryBase {
  /// Subtracts another value.
  BinaryBase operator -(BinaryBase other) => this;
}

/// Unary negation does not override binary subtraction.
class UnaryFromBinary extends BinaryBase {
  @override
  UnaryFromBinary operator -() => this; /* expect: public-member-api-docs */
}

/// Operator return type used through a prefix containing `operator`.
class Vec {}
