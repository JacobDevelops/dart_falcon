// Test cases for prefer-moving-to-variable rule
// Repeated property/getter chains can share a local binding.

void duplicateGetterChain() {
  final a = service.configuration.value;
  final b = service.configuration.value; /* expect: prefer-moving-to-variable */
}

void duplicateFieldAccess() {
  final x = obj.property.value;
  final y = obj.property.value; /* expect: prefer-moving-to-variable */
}

class Processor {
  void process() {
    final first = this.data.value;
    final second = this.data.value; /* expect: prefer-moving-to-variable */
    combine(first, second);
  }
}

void duplicateNullAwareChain() {
  final x = obj?.property?.value;
  final y = obj?.property?.value; /* expect: prefer-moving-to-variable */
}

void duplicateAcrossReadOnlyDeclaration() {
  final v1 = obj.property.value;
  final unrelated = 42;
  final v2 = obj.property.value; /* expect: prefer-moving-to-variable */
}

void duplicateAcrossPlainString() {
  final before = obj.property.value;
  final separator = ', ';
  final after = obj.property.value; /* expect: prefer-moving-to-variable */
}

void duplicateMethodChain() {
  final chain1 = obj.method().chain().result;
  final chain2 = obj.method().chain().result; /* expect: prefer-moving-to-variable */
}

void duplicateReceiverInvocation() {
  final filtered1 = list.where((e) => e > 5).toList();
  final filtered2 = list.where((e) => e > 5).toList(); /* expect: prefer-moving-to-variable */
}

void duplicateThemeLookup() {
  final first = Theme.of(context).textTheme;
  final second = Theme.of(context).textTheme; /* expect: prefer-moving-to-variable */
}

void duplicateReceiverCall() {
  final first = service.nextValue();
  final second = service.nextValue(); /* expect: prefer-moving-to-variable */
}

void unrelatedNonLateName() {
  final shared = 1;
  final first = o.s;
  final unrelated = shared;
  final second = o.s; /* expect: prefer-moving-to-variable */
}

void pureLateRead() {
  late final m = 1;
  final first = o.s;
  final unrelated = m;
  final second = o.s; /* expect: prefer-moving-to-variable */
}

class StaticLookup {
  static Object of(Object context) => context;
}

void duplicateKnownStaticMethod(Object context) {
  final first = StaticLookup.of(context);
  final second = StaticLookup.of(context); /* expect: prefer-moving-to-variable */
}

void capturedLateShadowedByParameter() {
  late final m = mutate();
  void inner(int m) {
    final a = o.s;
    final b = m;
    final c = o.s; /* expect: prefer-moving-to-variable */
  }
  inner(1);
}

void capturedLateShadowedByLocal() {
  late final m = mutate();
  final inner = () {
    final m = 1;
    final a = o.s;
    final b = m;
    final c = o.s; /* expect: prefer-moving-to-variable */
  };
  inner();
}

void creatingClosureDoesNotReadLate() {
  late final m = mutate();
  final a = o.s;
  final inner = () => m;
  final c = o.s; /* expect: prefer-moving-to-variable */
}

void lateSwitchCaseScopes(int value) {
  switch (value) {
    case 0:
      late final m = mutate();
      final a = o.s;
      final b = m;
      final c = o.s;
      break;
    default:
      final m = 1;
      final a = o.s;
      final b = m;
      final c = o.s; /* expect: prefer-moving-to-variable */
  }
}

void lateTryCatchFinallyScopes() {
  try {
    late final m = mutate();
    final a = o.s;
    final b = m;
    final c = o.s;
  } catch (error) {
    final m = 1;
    final a = o.s;
    final b = m;
    final c = o.s; /* expect: prefer-moving-to-variable */
  } finally {
    final m = 1;
    final a = o.s;
    final b = m;
    final c = o.s; /* expect: prefer-moving-to-variable */
  }
}
