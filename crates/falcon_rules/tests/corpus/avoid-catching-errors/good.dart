void work() {}

class MyException implements Exception {}

class Unrelated {}

void catchesException() {
  try {
    work();
  } on Exception catch (e) {
    print(e);
  }
}

void catchesFormatException() {
  try {
    work();
  } on FormatException catch (e) {
    print(e);
  }
}

void catchesCustomException() {
  try {
    work();
  } on MyException catch (e) {
    print(e);
  }
}

void catchesUnrelated() {
  try {
    work();
  } on Unrelated catch (e) {
    print(e);
  }
}

void catchesObject() {
  try {
    work();
  } on Object catch (e) {
    print(e);
  }
}

void bareCatch() {
  try {
    work();
  } catch (e) {
    print(e);
  }
}
