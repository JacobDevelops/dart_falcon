import 'package:flutter/material.dart'; /* expect: unnecessary-flutter-imports */
import 'package:flutter/foundation.dart'; /* expect: unnecessary-flutter-imports */
import 'dart:async'; /* expect: unnecessary-flutter-imports */
import 'package:flutter/services.dart'; /* expect: unnecessary-flutter-imports */
import 'package:flutter/widgets.dart'; /* expect: unnecessary-flutter-imports */
import 'package:flutter/material.dart' as flutter show Text; /* expect: unnecessary-flutter-imports */

void main() {
  print('Hello World');
}

class MyApp {
  void doSomething() {
    print('No Flutter symbols used');
  }
}
