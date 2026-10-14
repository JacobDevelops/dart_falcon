// Test cases for prefer-moving-to-variable rule
// No violations: unsupported initializers or chains separated by mutation barriers.

void distinctComputations() {
  final a = calculateExpensive(10);
  final b = calculateExpensive(20);
}

void singleUse() {
  final result = getResult(data);
  print(result);
}

// Trivial literals are not flagged
void trivialLiterals() {
  final x = 42;
  final y = 42;
  final s1 = "hello";
  final s2 = "hello";
}

class Processor {
  void process() {
    final first = getResult(dataA);
    final second = getResult(dataB);
    combine(first, second);
  }
}

// Only one assignment of complex expression
void singleComplexAssignment() {
  final x = obj.method().chain().result;
  print(x);
}

// Identifiers (trivial) don't trigger violation
void identifierDuplicates() {
  final a = variable1;
  final b = variable1;
}

// Different chained calls
void differentChains() {
  final c1 = obj.method1().result;
  final c2 = obj.method2().result;
}

void separateConstructors() {
  final first = Completer();
  final second = Completer();
  final namedFirst = Completer.sync();
  final namedSecond = Completer.sync();
}

void separateCalls() {
  final first = calculateExpensive(10);
  final second = calculateExpensive(10);
  final value1 = service.nextValue();
  final value2 = service.nextValue();
  final chain1 = obj.method().chain().result;
  final chain2 = obj.method().chain().result;
  final filtered1 = list.where((e) => e > 5).toList();
  final filtered2 = list.where((e) => e > 5).toList();
}

Future<void> separateAwaits() async {
  final first = await service.load();
  final second = await service.load();
}

void freshCollections() {
  final list1 = <int>[];
  final list2 = <int>[];
  final map1 = <String, int>{};
  final map2 = <String, int>{};
  final set1 = <int>{};
  final set2 = <int>{};
  final completers1 = [Completer()];
  final completers2 = [Completer()];
}

void separateOperators() {
  final first = a + b;
  final second = a + b;
}

void changingState() {
  final before = notifier.state;
  notifier.state = nextState;
  final after = notifier.state;
}

void changingThroughCall() {
  final before = LoggerService.instance;
  configureLogger();
  final after = LoggerService.instance;
}

void effectfulInitializer() {
  final before = notifier.state;
  final mutation = notifier.refresh();
  final after = notifier.state;
}

Future<void> stateAcrossAwait() async {
  final before = notifier.state;
  await refresh();
  final after = notifier.state;
}

void nestedMutation() {
  final before = notifier.state;
  if (condition) {
    notifier.state = nextState;
  }
  final after = notifier.state;
}

void mutatedInSameDeclaration() {
  final before = notifier.state,
      mutation = notifier.refresh(),
      after = notifier.state;
}

void reboundRoot() {
  final before = notifier.state;
  notifier = nextNotifier;
  final after = notifier.state;
}

final repeatedConstructorsInCallback = () {
  final first = Completer();
  final second = Completer();
};

Future<void> stateAcrossAwaitInitializer() async {
  final before = notifier.state;
  final loaded = await refresh();
  final after = notifier.state;
}

void deferredReads() {
  final before = notifier.state;
  late final after = notifier.state;
  notifier.state = nextState;
  use(before, after);
}

void separateDeferredReads() {
  late final before = notifier.state;
  late final after = notifier.state;
  use(before);
  notifier.state = nextState;
  use(after);
}

void deferredDeclarationBarrier() {
  final before = notifier.state;
  late final deferred = notifier.state;
  final after = notifier.state;
}

void interpolatedInitializerBarrier() {
  final before = notifier.state;
  final text = '${notifier.refresh()}';
  final after = notifier.state;
}
