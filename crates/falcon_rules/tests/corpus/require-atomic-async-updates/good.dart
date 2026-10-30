int count = 0;
int other = 0;
class Counter {
  int value = 0;
  Future<void> update(Counter other, Future<int> amount) async {
    value = other.value + await amount;
    this.value = other.value + await amount;
    this.value = await amount + this.value;
    final local = await amount;
    value += local;
  }
  Future<void> patterns(Object subject, Future<int> amount) async {
    value = switch (subject) {
      Counter(:final value) => value + await amount,
      _ => 0,
    };
    value = [if (subject case Counter(:final value)) value + await amount else 0].first;
    value = [for (final value in [0]) value + await amount].first;
    value = [for (var value = 0; value < 1; value++) value + await amount].first;
    // Conservative fallback: a target-name binding suppresses new reads in
    // that scope, even when an explicitly qualified target is still visible.
    this.value = switch (subject) {
      Counter(:final value) => this.value + await amount,
      _ => 0,
    };
    this.value = [if (subject case Counter(:final value)) this.value + await amount else 0].first;
    this.value = [for (final value in [0]) this.value + await amount].first;
  }
}
Future<void> update(Future<int> amount, bool condition) async {
  final local = await amount;
  count += local;
  count = await amount;
  count = await amount + count;
  count = other + await amount;
  count = condition ? count : await amount;
  count = (() { return count + 1; })();
  Future<int> later() async => count + await amount;
  count = await later();
  final values = <int>[0, 1];
  values[0] = values[1] + await amount;
  values[0] = await amount + values[0];
  {
    var count = 0;
    count = await amount;
  }
}

Future<void> branches(
  Future<int> amount,
  Object subject,
  bool condition,
) async {
  count = switch (subject) { 0 => count, _ => await amount };
  count = switch (subject) { 0 => await amount, _ => count };
  count = switch (subject) { int count => count + await amount, _ => 0 };
  count = switch (subject) { int count when count > await amount => count, _ => 0 };
  count = [if (condition) count else await amount].first;
  count = [if (condition) await amount else count].first;
  count = [if (subject case int count) count + await amount else 0].first;
  count = [if (subject case int count when count > await amount) count else 0].first;
  count = {if (condition) 0: count else 1: await amount}.values.first;
  count = {if (condition) count else await amount}.first;
  count = {if (subject case int count) 0: count + await amount else 1: 0}.values.first;
  count = [for (final count in [0, 1]) count + await amount].first;
  count = [for (var count = 0; count < 2; count++) count + await amount].first;
  count = [for (final (count,) in [(0,), (1,)]) count + await amount].first;
  count = {for (final count in [0, 1]) count: count + await amount}.values.first;
  count = switch (subject) { int count => count, _ => 0 } + await amount;
  count = [if (subject case int count) count else 0].first + await amount;
  count = [for (final count in [0]) count].first + await amount;
}

Future<void> identities(Future<int> amount, dynamic receiver) async {
  receiver.x.count = receiver.y.count + await amount;
  receiver.x.count = receiver.x.other + await amount;
  receiver.x[0] = receiver.y[0] + await amount;
  receiver.x.count = await amount + receiver.x.count;
  final values = <int>[0, 1];
  final otherValues = <int>[0, 1];
  var index = 0;
  var otherIndex = 1;
  values[index] = otherValues[index] + await amount;
  values[index] = values[otherIndex] + await amount;
  values[index] = switch (other) { int index => values[index] + await amount, _ => 0 };
  values[index] = [if (other case int index) values[index] + await amount else 0].first;
  values[index] = [for (final index in [0]) values[index] + await amount].first;
  values[index] = [for (final values in [otherValues]) values[index] + await amount].first;
  count = (count = await amount);
  count = (values[0] = await amount);
  count = (Counter().value = await amount);
}

Future<void> lazyInitialization(Future<int> amount) async {
  final value = await amount;
  int? cached;
  cached ??= value;
  cached = cached ?? value;
  cached = await amount;
}

void synchronous() {
  count = count + 1;
  final update = () => count = count + 1;
  update();
}

Future<void> closureBoundary(Future<int> amount) async {
  // Merely constructing an asynchronous closure does not execute its reads.
  dynamic callback;
  callback = () async => callback + await amount;
  final synchronous = () {
    count = count + 1;
  };
  synchronous();
}

Future<void> localLoop(List<Future<int>> futures, int parameter) async {
  var total = 0;
  for (final f in futures) {
    total += await f;
    parameter += await f;
  }
}
Future<void> shadowedCapture(Future<int> amount) async {
  var local = 0;
  final read = (int local) => local;
  local += await amount;
}
