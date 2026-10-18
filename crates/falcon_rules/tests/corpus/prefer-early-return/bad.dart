void top(bool ready) {
  if (ready) { /* expect: prefer-early-return */
    print('ready');
    print('work');
  }
}
void afterWork(bool ready) {
  print('start');
  if (ready) { /* expect: prefer-early-return */
    print('ready');
    print('work');
  }
}
class Worker {
  Worker(bool ready) {
    if (ready) { /* expect: prefer-early-return */
      print('ready');
      print('work');
    }
  }
  void run(bool ready) {
    if (ready) { /* expect: prefer-early-return */
      print('ready');
      print('work');
    }
  }
  set ready(bool value) {
    if (value) { /* expect: prefer-early-return */
      print('ready');
      print('work');
    }
  }
}
void outer() {
  void local(bool ready) {
    if (ready) { /* expect: prefer-early-return */
      print('ready');
      print('work');
    }
  }
  local(true);
}
void nested(bool first, bool second) {
  if (first) { /* expect: prefer-early-return */
    if (second) { print('one'); print('two'); }
  }
}
set ready(bool value) {
  if (value) { /* expect: prefer-early-return */
    print('one'); print('two');
  }
}
void asynchronous(bool value) async {
  if (value) { /* expect: prefer-early-return */
    print('one'); print('two');
  }
}
class Indexed {
  void operator []=(int index, bool value) {
    if (value) { /* expect: prefer-early-return */
      print(index); print(value);
    }
  }
}
