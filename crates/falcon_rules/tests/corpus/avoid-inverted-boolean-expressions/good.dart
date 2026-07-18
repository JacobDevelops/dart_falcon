class BooleanExpressions {
  void examples() {
    bool isValid = true;
    bool condition = false;

    /// Direct boolean check
    if (isValid) {
      print('valid');
    }

    /// Single negation
    if (!condition) {
      print('condition met');
    }

    /// Direct assignment
    final result = isValid;
    print(result);

    /// Single negation in assignment
    var flag = !condition;

    /// Single negation in return
    if (isValid) {
      return;
    }

    /// Simple negation
    bool x = !isValid;
  }
}
