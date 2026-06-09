class BooleanExpressions {
  void examples() {
    bool isValid = true;
    bool condition = false;

    /// Double negation with double bang
    if (!!isValid) { /* expect: avoid_inverted_boolean_expressions */
      print('valid');
    }

    /// Nested negation
    if (!(!condition)) { /* expect: avoid_inverted_boolean_expressions */
      print('condition met');
    }

    /// Double negation in assignment
    final result = !!isValid; /* expect: avoid_inverted_boolean_expressions */
    print(result);

    /// Double negation in variable declaration
    var flag = !!condition; /* expect: avoid_inverted_boolean_expressions */

    /// Negation of negation in return
    if (!(!isValid)) { /* expect: avoid_inverted_boolean_expressions */
      return;
    }

    /// Triple negation (still bad)
    bool x = !!!isValid; /* expect: avoid_inverted_boolean_expressions */
  }
}
