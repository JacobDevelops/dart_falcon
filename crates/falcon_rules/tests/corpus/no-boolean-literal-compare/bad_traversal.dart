extension type Flag(bool value) {
  bool check(bool enabled) {
    final record = (enabled == true,); /* expect: no-boolean-literal-compare */
    return switch (value) {
      true => (enabled != false,).$1, /* expect: no-boolean-literal-compare */
      false => false,
    };
  }
}

void scopedStatementRegions(Object value) {
  try {
    final bool tryFlag = true;
    if (tryFlag == true) {} /* expect: no-boolean-literal-compare */
  } catch (_) {
    final bool catchFlag = true;
    if (catchFlag != false) {} /* expect: no-boolean-literal-compare */
  } finally {
    if (tryFlag == true) {}
    if (catchFlag == true) {}
  }

  if (value case bool matched when matched == true) { /* expect: no-boolean-literal-compare */
    if (matched != false) {} /* expect: no-boolean-literal-compare */
  } else {
    if (matched == true) {}
  }
  if (matched == true) {}
}
