// Good: using ternary/conditional expressions
String getStatus(bool isActive) {
  return isActive ? 'active' : 'inactive';
}

int getValue(bool hasValue) {
  return hasValue ? 42 : 0;
}

class Validator {
  bool isValid(String input) {
    return input.isNotEmpty;
  }
}

// OK: complex logic in if/else (more than simple return)
void processData(bool flag) {
  if (flag) {
    print('Processing...');
    final result = compute();
    save(result);
  } else {
    cleanup();
  }
}

// OK: if with additional statements
String getMessage(bool success) {
  if (success) {
    log('Success');
    return 'Done!';
  }
  return 'Failed!';
}
