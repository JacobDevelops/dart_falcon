class Concrete {
  void run() {}
}

abstract class WithField {
  int get id;
  void run();
}

abstract class TwoMethods {
  void start();
  void stop();
}

abstract class Base {}

abstract class Extending extends Base {
  void run();
}

abstract class Implementing implements Base {
  void run();
}

abstract class WithBody {
  void run() {}
}

abstract class WithState {
  int count = 0;
  void run();
}

abstract class WithSetter {
  set value(int v);
  void run();
}

abstract class Empty {}
