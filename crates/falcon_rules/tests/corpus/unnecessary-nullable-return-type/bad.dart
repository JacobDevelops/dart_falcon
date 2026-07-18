// Bad: an outer-nullable return type whose every return is a non-null literal.

String? getStatus() { /* expect: unnecessary-nullable-return-type */
  return 'active';
}

int? getNumber() { /* expect: unnecessary-nullable-return-type */
  return 42;
}

// The outer `?` (after `List<...>`) counts; the returned list is non-null.
List<String?>? getItems() { /* expect: unnecessary-nullable-return-type */
  return ['a', 'b', 'c'];
}

bool? isValid() { /* expect: unnecessary-nullable-return-type */
  if (DateTime.now().isUtc) {
    return true;
  }
  return false;
}
