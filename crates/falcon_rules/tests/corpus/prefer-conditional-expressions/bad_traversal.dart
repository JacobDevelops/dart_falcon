final callback = () {
  if (true) { /* expect: prefer-conditional-expressions */
    return 1;
  } else {
    return 2;
  }
};

void localFunction() {
  int nested(bool condition) {
    if (condition) { /* expect: prefer-conditional-expressions */
      return 1;
    } else {
      return 2;
    }
  }
}

int labeled(bool condition) {
  label:
  if (condition) { /* expect: prefer-conditional-expressions */
    return 1;
  } else {
    return 2;
  }
}

void closureInCondition(bool condition) {
  if (check(() {
    if (condition) { /* expect: prefer-conditional-expressions */
      return true;
    } else {
      return false;
    }
  })) {}
}
