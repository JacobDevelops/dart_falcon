// Bad: a single assignment or single return in each branch.
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

void setLabel(bool on) {
  String label;
  if (on) { /* expect: prefer-conditional-expressions */
    label = 'on';
  } else {
    label = 'off';
  }
  print(label);
}
