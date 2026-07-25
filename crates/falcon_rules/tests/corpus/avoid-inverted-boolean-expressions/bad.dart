class BooleanExpressions {
  void examples() {
    bool isValid = true;
    bool condition = false;

    /// Double negation with double bang
    if (!!isValid) { /* expect: avoid-inverted-boolean-expressions */
      print('valid');
    }

    /// Nested negation
    if (!(!condition)) { /* expect: avoid-inverted-boolean-expressions */
      print('condition met');
    }

    /// Double negation in assignment
    final result = !!isValid; /* expect: avoid-inverted-boolean-expressions */
    print(result);

    /// Double negation in variable declaration
    var flag = !!condition; /* expect: avoid-inverted-boolean-expressions */

    /// Negation of negation in return
    if (!(!isValid)) { /* expect: avoid-inverted-boolean-expressions */
      return;
    }

    /// Triple negation (still bad)
    bool x = !!!isValid; /* expect: avoid-inverted-boolean-expressions */
  }
}

// Regression: the violation must still be found inside Dart 3 containers
// (pattern declaration, labeled statement, switch expression and subject,
// collection if/spread, record field).
void containersRegression(int rcount, bool flag) {
  final (ra, _) = (!!flag, 0); /* expect: avoid-inverted-boolean-expressions */
  lbl: {
    final rb = !!flag; /* expect: avoid-inverted-boolean-expressions */
    print(rb);
  }
  final rc = switch (rcount) {
    0 => !!flag, /* expect: avoid-inverted-boolean-expressions */
    _ => null,
  };
  final rd = switch (!!flag) { /* expect: avoid-inverted-boolean-expressions */
    _ => 0,
  };
  final re = [if (rcount > 0) !!flag]; /* expect: avoid-inverted-boolean-expressions */
  final rf = [...[!!flag]]; /* expect: avoid-inverted-boolean-expressions */
  final rg = (p: !!flag, q: 0); /* expect: avoid-inverted-boolean-expressions */
  print([ra, rc, rd, re, rf, rg]);
}
