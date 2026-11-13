// @dart = 3.7
bool? _;

void parameter(bool _) {
  print(_ == true);
}

void local() {
  bool _ = true;
  print(_ == true);
}

void caught() {
  try {
    throw true;
  } catch (_) {
    print(_ == true);
  }
}
