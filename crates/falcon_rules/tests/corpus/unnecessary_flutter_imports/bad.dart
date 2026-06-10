import 'package:flutter/material.dart'; /* expect: unnecessary_flutter_imports */
import 'package:flutter/foundation.dart'; /* expect: unnecessary_flutter_imports */
import 'dart:async'; /* expect: unnecessary_flutter_imports */

void main() {
  print('Hello World');
}

class MyApp {
  void doSomething() {
    print('No Flutter symbols used');
  }
}
