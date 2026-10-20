void f(dynamic value, dynamic next) {
  value..first = 1..second = 1;
  value..[0] = 1..[1] = 1;
  value..child.name = 'a'..other.name = 'a';
  value..reset()..reset();
  value..set(1)..set(2);
  value..count += 1..count += 1;
  value..[next()] = 1..[next()] = 2;
  value..child().name = 1..child().name = 2;
}
void stateful(dynamic writer, dynamic value, int i) {
  writer..indent = 2..writeln('x')..indent = 0;
  value..[++i] = 1..[++i] = 2;
  value..[--i] = 1..[--i] = 2;
}

void rhsMutations(List<int> xs) {
  var i = 0;
  xs..[i] = i++..[i] = 9;
  xs..[i] = i--..[i] = 9;
  xs..[i] = ++i..[i] = 9;
  xs..[i] = --i..[i] = 9;
  xs..[i] = (i = 1)..[i] = 9;
  xs..[i] = (i += 1)..[i] = 9;
}

class Counter {
  int i = 0;
  void increment() => i++;
}

void nestedMutations(List<int> xs, Counter counter) {
  xs..[counter.i] = (counter..i = 1).i..[counter.i] = 9;
  xs..[counter.i] = (counter..increment()).i..[counter.i] = 9;
  xs..[(counter..i += 1).i] = 1..[(counter..i += 1).i] = 2;
}
