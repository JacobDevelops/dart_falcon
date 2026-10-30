int count = 0;
class Counter {
  int value = 0;
  int? cached;
  final items = <int>[0];
  late Counter child;
  Future<void> update(Future<int> amount) async {
    value += await amount; /* expect: require-atomic-async-updates */
    this.value *= await amount; /* expect: require-atomic-async-updates */
    this.value = this.value + await amount; /* expect: require-atomic-async-updates */
    value = this.value + await amount; /* expect: require-atomic-async-updates */
    this.value = value + await amount; /* expect: require-atomic-async-updates */
    this.items[0] = this.items[0] + await amount; /* expect: require-atomic-async-updates */
    items[0] = this.items[0] + await amount; /* expect: require-atomic-async-updates */
    this.items[0] = items[0] + await amount; /* expect: require-atomic-async-updates */
    this.child.value = this.child.value + await amount; /* expect: require-atomic-async-updates */
    child.value = this.child.value + await amount; /* expect: require-atomic-async-updates */
    this.child.value = child.value + await amount; /* expect: require-atomic-async-updates */
    cached ??= await amount; /* expect: require-atomic-async-updates */
    this.cached = cached ?? await amount; /* expect: require-atomic-async-updates */
  }
}
Future<void> update(Future<int> amount) async {
  count += await amount; /* expect: require-atomic-async-updates */
  count = count + await amount; /* expect: require-atomic-async-updates */
  count = (count * 2) - await amount; /* expect: require-atomic-async-updates */
  count = await Future.value(count); /* expect: require-atomic-async-updates */
  var local = 0;
  local -= await amount;
  count = true ? count + await amount : 0; /* expect: require-atomic-async-updates */
  final values = <int>[0];
  values[0] += await amount; /* expect: require-atomic-async-updates */
  values[0] = values[0] + await amount; /* expect: require-atomic-async-updates */
}

Future<void> branches(
  Future<int> amount,
  Object subject,
  bool condition,
) async {
  count = switch (subject) { 0 => count + await amount, _ => 0 }; /* expect: require-atomic-async-updates */
  count = switch (subject) { int n => count + n + await amount, _ => 0 }; /* expect: require-atomic-async-updates */
  count = switch (subject) { int n when count > 0 => n + await amount, _ => 0 }; /* expect: require-atomic-async-updates */
  count = switch (subject) { int n when count < 0 => n, _ => await amount }; /* expect: require-atomic-async-updates */
  count = count + switch (subject) { int count => count + await amount, _ => 0 }; /* expect: require-atomic-async-updates */
  count = switch (count) { 0 => await amount, _ => 0 }; /* expect: require-atomic-async-updates */
  count = switch (subject) { int count => count, _ => count + await amount }; /* expect: require-atomic-async-updates */
  count = [if (condition) count + await amount else 0].first; /* expect: require-atomic-async-updates */
  count = [if (subject case int n) count + n + await amount else 0].first; /* expect: require-atomic-async-updates */
  count = [if (subject case int n when count > 0) n + await amount else 0].first; /* expect: require-atomic-async-updates */
  count = [if (condition) count, await amount].first; /* expect: require-atomic-async-updates */
  count = {if (condition) 0: count + await amount else 1: 0}.values.first; /* expect: require-atomic-async-updates */
  count = {if (condition) count + await amount else 0}.first; /* expect: require-atomic-async-updates */
  count = count + [if (subject case int count) count + await amount else 0].first; /* expect: require-atomic-async-updates */
  count = [if (subject case int count) count else count + await amount].first; /* expect: require-atomic-async-updates */
  count = [for (final n in [0, 1]) count + n + await amount].first; /* expect: require-atomic-async-updates */
  count = [for (var n = 0; n < 2; n++) count + n + await amount].first; /* expect: require-atomic-async-updates */
  count = [for (final (n,) in [(0,), (1,)]) count + n + await amount].first; /* expect: require-atomic-async-updates */
  count = [for (final n in [0, 1]) if (n == 0) count else await amount].first; /* expect: require-atomic-async-updates */
  count = [for (final n in [count]) await amount].first; /* expect: require-atomic-async-updates */
  count = count + [for (final count in [0]) count + await amount].first; /* expect: require-atomic-async-updates */
  count = [for (final count in [0]) count, count + await amount].first; /* expect: require-atomic-async-updates */
  count = {for (final n in [0, 1]) n: count + await amount}.values.first; /* expect: require-atomic-async-updates */
  count = {count: await amount}.values.first; /* expect: require-atomic-async-updates */
}

Future<void> identities(Future<int> amount, dynamic receiver) async {
  receiver.x.count = receiver.x.count + await amount; /* expect: require-atomic-async-updates */
  receiver.x[0] = receiver.x[0] + await amount; /* expect: require-atomic-async-updates */
  final values = <int>[0, 1];
  var index = 0;
  values[index] = values[index] + await amount; /* expect: require-atomic-async-updates */
  count = (values[count] = await amount); /* expect: require-atomic-async-updates */
  dynamic object = Counter();
  object = (object.value = await amount);
}

Future<void> lazyInitialization(Future<int> amount) async {
  int? cached;
  cached ??= await amount;
  cached = cached ?? await amount;
  int? local;
  local = local ?? await amount;
}

Future<void> asynchronousLoop(Stream<int> amounts) async {
  count = [await for (final amount in amounts) count + amount].first; /* expect: require-atomic-async-updates */
}

Future<void> closures(Future<int> amount) async {
  final update = () async {
    count = count + await amount; /* expect: require-atomic-async-updates */
  };
  await update();
}

Stream<int> generator(Future<int> amount) async* {
  count += await amount; /* expect: require-atomic-async-updates */
  yield count;
}

Future<void> capturedLocal(Future<int> amount, int parameter) async {
  var local = 0;
  final read = () => local;
  final readParameter = () => parameter;
  local += await amount; /* expect: require-atomic-async-updates */
  parameter += await amount; /* expect: require-atomic-async-updates */
}
Future<void> capturedByLocalFunction(Future<int> amount) async {
  var local = 0;
  int read() => local;
  local += await amount; /* expect: require-atomic-async-updates */
}
class StaticCounter {
  static int value = 0;
  static Future<void> update(Future<int> amount) async {
    value += await amount; /* expect: require-atomic-async-updates */
  }
}

Future<void> capturedByWrites(Future<int> amount) async {
  var local = 0;
  void reset() { (local, _) = (0, 0); }
  local += await amount; /* expect: require-atomic-async-updates */
}
Future<void> capturedByLoop(Future<int> amount, List<int> values) async {
  var local = 0;
  void reset() { for (local in values) {} }
  local += await amount; /* expect: require-atomic-async-updates */
}
Future<void> capturedByAssignment(Future<int> amount) async {
  var local = 0;
  void reset() { local = 0; }
  local += await amount; /* expect: require-atomic-async-updates */
}
void asyncClosureAssignment(Future<int> amount) {
  var local = 0;
  final update = () async {
    local += await amount; /* expect: require-atomic-async-updates */
  };
}
