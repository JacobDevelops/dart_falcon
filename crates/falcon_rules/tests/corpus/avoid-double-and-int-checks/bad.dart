void handle(num x) {
  if (x is double) {
    print('double');
  } else if (x is int) { /* expect: avoid-double-and-int-checks */
    print('int');
  }
}

void handleDynamic(dynamic value) {
  if (value is double) {
    print('double');
  } else if (value is int) { /* expect: avoid-double-and-int-checks */
    print('int');
  } else {
    print('other');
  }
}

void handleLocal(Object input) {
  final number = input;
  if (number is double) {
    print('double');
  } else if (number is int) { /* expect: avoid-double-and-int-checks */
    print('int');
  }
}

void handleOptional([Object? x]) {
  if (x is double) print('double'); else if (x is int) print('int'); /* expect: avoid-double-and-int-checks */
}

void handleNamed({required num x}) {
  if (x is double) {
    print('double');
  } else if (x is int) { /* expect: avoid-double-and-int-checks */
    print('int');
  }
}

final closure = (num x) {
  if (x is double) {
    return 1;
  } else if (x is int) { /* expect: avoid-double-and-int-checks */
    return 2;
  }
  return 0;
};
