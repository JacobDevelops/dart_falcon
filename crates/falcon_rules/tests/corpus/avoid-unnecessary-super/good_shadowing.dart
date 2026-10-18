import 'dart:developer';

int count = 100;

class Base {
  void log(String message) {}
  int get count => 7;
}

final class Child extends Base {
  void run() => super.log('x');
  int read() => super.count;
}
