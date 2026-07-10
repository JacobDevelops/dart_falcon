// Good: cases dart_code_linter's prefer-conditional-expressions does not flag.

String getStatus(bool isActive) {
  return isActive ? 'active' : 'inactive';
}

// Branches are calls, not a single assignment or return.
void printMessage(bool success) {
  if (success) {
    print('Success!');
  } else {
    print('Failed!');
  }
}

// else-if chains are skipped.
String classify(int code) {
  if (code == 0) {
    return 'zero';
  } else if (code == 1) {
    return 'one';
  } else {
    return 'many';
  }
}

// Assignments target different variables.
void assignDifferent(bool flag) {
  String x = '';
  String y = '';
  if (flag) {
    x = 'a';
  } else {
    y = 'b';
  }
  print('$x$y');
}

// Assignment target is not a simple identifier.
void assignIndex(bool flag, Map<String, int> m) {
  if (flag) {
    m['a'] = 1;
  } else {
    m['b'] = 2;
  }
}

// Multiple statements per branch.
void processData(bool flag) {
  if (flag) {
    print('Processing...');
    final result = compute();
    save(result);
  } else {
    cleanup();
  }
}
