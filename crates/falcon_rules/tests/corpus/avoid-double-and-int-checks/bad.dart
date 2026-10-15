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

void negated(num x) {
  if (x is! double) {
    print('not double');
  } else if (x is int) { /* expect: avoid-double-and-int-checks */
    print('int');
  }
}

void negatedElse(num x) {
  if (x is double) {
    print('double');
  } else if (x is! int) { /* expect: avoid-double-and-int-checks */
    print('not int');
  }
}

class ShadowedFieldChecks {
  num field = 1;

  void parameter(num field) {
    if (field is double) {
      print('double');
    } else if (field is int) { /* expect: avoid-double-and-int-checks */
      print('int');
    }
  }

  void local() {
    final field = this.field;
    if (field is double) {
      print('double');
    } else if (field is int) { /* expect: avoid-double-and-int-checks */
      print('int');
    }
  }
}
