class Counter {
  static int count = 0;
  int value = 0;
  static set setting(int value) {}
  void change(int count) {
    count = 1;
    value = 2;
    value++;
    this.value += 2;
  }
  void local() {
    int count = 0;
    count++;
  }
  static void reset() {
    count = 0;
    Counter.count += 1;
    count++;
  }
}
void change(Counter counter) {
  counter.value = 3;
  Counter.setting = 1;
  final read = Counter.count;
  Counter.count = 2;
  Counter.count--;
}
