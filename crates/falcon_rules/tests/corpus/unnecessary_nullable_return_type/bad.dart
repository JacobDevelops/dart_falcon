// Bad: an outer-nullable return type whose every return is a non-null literal.

String? getStatus() { /* expect: unnecessary_nullable_return_type */
  return 'active';
}

int? getNumber() { /* expect: unnecessary_nullable_return_type */
  return 42;
}

// The outer `?` (after `List<...>`) counts; the returned list is non-null.
List<String?>? getItems() { /* expect: unnecessary_nullable_return_type */
  return ['a', 'b', 'c'];
}

bool? isValid() { /* expect: unnecessary_nullable_return_type */
  if (DateTime.now().isUtc) {
    return true;
  }
  return false;
}
