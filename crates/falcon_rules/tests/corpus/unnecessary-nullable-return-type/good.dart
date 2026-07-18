// Good: return type is non-nullable when function never returns null

Future<String> getName() async {
  return 'hello';
}

String getStatus() {
  return 'active';
}

int getNumber() {
  return 42;
}

List<String> getItems() {
  return ['a', 'b', 'c'];
}

Future<bool> isValid() async {
  return true;
}

// Good: nullable return type when function can return null

String? maybeGetName(bool shouldReturn) {
  if (shouldReturn) {
    return 'hello';
  }
  return null;
}

int? findIndex(List<int> items, int target) {
  try {
    return items.indexOf(target);
  } catch (e) {
    return null;
  }
}

// Good: only the OUTER `?` counts — a nullable type argument does not.
Future<String?> asyncName() async {
  return 'hello';
}

Future<bool?> asyncValid() async {
  return true;
}

// Good: returns we cannot prove non-null are not flagged.
String? fromVariable(String value) {
  return value;
}

String? fromCall() {
  return compute();
}

String? withNullBranch(bool flag) {
  if (flag) {
    return 'x';
  }
  return null;
}
