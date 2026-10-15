import 'operators.dart' as operators;

/// Value.
class Value {
  Value operator +(Value other) => this; /* expect: public-member-api-docs */
  bool operator ==(Object other) => false;
  int get hashCode => 0;
}

/// Annotation text and return-type identifiers are not the operator keyword.
class OperatorLocations {
  @Deprecated('use operator - instead')
  operators.Vec operator +(operators.Vec other) => operators.Vec(); /* expect: public-member-api-docs */
}
