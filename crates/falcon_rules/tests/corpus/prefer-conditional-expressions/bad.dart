// Bad: simple if/else that could be ternary
String getStatus(bool isActive) {
  if (isActive) { /* expect: prefer-conditional-expressions */
    return 'active';
  } else {
    return 'inactive';
  }
}

int getValue(bool hasValue) {
  if (hasValue) /* expect: prefer-conditional-expressions */
    return 42;
  else
    return 0;
}

class Validator {
  bool isValid(String input) {
    if (input.isNotEmpty) { /* expect: prefer-conditional-expressions */
      return true;
    } else {
      return false;
    }
  }
}

void printMessage(bool success) {
  if (success) { /* expect: prefer-conditional-expressions */
    print('Success!');
  } else {
    print('Failed!');
  }
}

void setLabel(bool on) {
  String label;
  if (on) { /* expect: prefer-conditional-expressions */
    label = 'on';
  } else {
    label = 'off';
  }
  print(label);
}
