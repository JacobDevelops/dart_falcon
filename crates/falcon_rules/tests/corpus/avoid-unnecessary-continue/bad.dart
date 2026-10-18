void classic() {
  for (var i = 0; i < 3; i++) {
    print(i);
    continue; /* expect: avoid-unnecessary-continue */
  }
}

void iterable(List<int> values) {
  for (final value in values) {
    print(value);
    continue; /* expect: avoid-unnecessary-continue */
  }
}

void waiting(bool ready) {
  while (ready) {
    continue; /* expect: avoid-unnecessary-continue */
  }
}

void once() {
  do {
    continue; /* expect: avoid-unnecessary-continue */
  } while (false);
}

Future<void> asynchronous(Stream<int> values) async {
  await for (final value in values) {
    print(value);
    continue; /* expect: avoid-unnecessary-continue */
  }
}

void unbraced(bool ready) {
  while (ready) continue; /* expect: avoid-unnecessary-continue */
}

void conditionalTail(List<int> values) {
  for (final value in values) {
    if (value < 0) {
      continue; /* expect: avoid-unnecessary-continue */
    } else {
      print(value);
      continue; /* expect: avoid-unnecessary-continue */
    }
  }
}
