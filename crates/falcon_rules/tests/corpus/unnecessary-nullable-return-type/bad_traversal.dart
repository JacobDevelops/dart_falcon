String? labeled(bool condition) { /* expect: unnecessary-nullable-return-type */
  result: {
    if (condition) {
      return 'yes';
    }
    return 'no';
  }
}

extension Values on int {
  String? describe() { /* expect: unnecessary-nullable-return-type */
    switch (this) {
      case 0:
        return 'zero';
      default:
        return 'other';
    }
  }
}

String? ignoresDelayedNullReturns() { /* expect: unnecessary-nullable-return-type */
  String? local() {
    return null;
  }
  final callback = () {
    return null;
  };
  return 'outer';
}

class NestedReturnScopes {
  String? get description { /* expect: unnecessary-nullable-return-type */
    final callback = () {
      return null;
    };
    return 'description';
  }
}
