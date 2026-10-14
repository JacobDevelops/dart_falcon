/// Value.
class Value {
  Value operator +(Value other) => this; /* expect: public-member-api-docs */
  bool operator ==(Object other) => false;
  int get hashCode => 0;
}
