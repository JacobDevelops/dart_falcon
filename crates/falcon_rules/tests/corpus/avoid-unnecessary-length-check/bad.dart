void inspect(List<int> values, Set<int> unique, Map<int, int> pairs) {
  if (values.isNotEmpty) { /* expect: avoid-unnecessary-length-check */
    for (final value in values) { print(value); }
  }
  if (values.length != 0) { /* expect: avoid-unnecessary-length-check */
    values.forEach((value) { print(value); });
  }
  if (unique.length > 0) { /* expect: avoid-unnecessary-length-check */
    for (final value in unique) { print(value); }
  }
  if (1 <= values.length) { /* expect: avoid-unnecessary-length-check */
    values.forEach((value) { print(value); });
  }
  if (pairs.isNotEmpty) { /* expect: avoid-unnecessary-length-check */
    pairs.forEach((key, value) { print(key); print(value); });
  }
  if (values.length >= 1) /* expect: avoid-unnecessary-length-check */
    for (final value in values) { print(value); }
}
