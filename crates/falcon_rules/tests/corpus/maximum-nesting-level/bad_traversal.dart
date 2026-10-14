class NestedOperator {
  @override
  bool operator ==(Object other) { /* expect: maximum-nesting-level */
    if (other is NestedOperator) {
      while (true) {
        if (hashCode == other.hashCode) { return true; }
        break;
      }
    }
    return false;
  }
}
