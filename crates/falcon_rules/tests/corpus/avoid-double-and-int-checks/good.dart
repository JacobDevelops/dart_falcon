num field = 1;

void intFirst(num x) {
  if (x is int) {
    print('int');
  } else if (x is double) {
    print('double');
  }
}

void differentVariables(num x, num y) {
  if (x is double) {
    print('double');
  } else if (y is int) {
    print('int');
  }
}

void notElseIf(num x) {
  if (x is double) {
    print('double');
  }
  if (x is int) {
    print('int');
  }
}

void fieldCheck() {
  if (field is double) {
    print('double');
  } else if (field is int) {
    print('int');
  }
}

void nullableTypes(Object? x) {
  if (x is double?) {
    print('double');
  } else if (x is int?) {
    print('int');
  }
}


void nestedBlock(num x) {
  if (x is double) {
    print('double');
  } else {
    if (x is int) {
      print('int');
    }
  }
}

class FieldChecks {
  num field = 1;

  void check() {
    if (field is double) {
      print('double');
    } else if (field is int) {
      print('int');
    }
  }
}
