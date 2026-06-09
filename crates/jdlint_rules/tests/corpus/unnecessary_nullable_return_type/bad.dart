// Bad: return type is nullable but function never returns null

Future<String?> getName() async { /* expect: unnecessary_nullable_return_type */
  return 'hello';
}

String? getStatus() { /* expect: unnecessary_nullable_return_type */
  return 'active';
}

int? getNumber() { /* expect: unnecessary_nullable_return_type */
  return 42;
}

List<String?>? getItems() { /* expect: unnecessary_nullable_return_type */
  return ['a', 'b', 'c'];
}

Future<bool?> isValid() async { /* expect: unnecessary_nullable_return_type */
  return true;
}
