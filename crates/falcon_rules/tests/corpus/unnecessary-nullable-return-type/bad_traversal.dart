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
