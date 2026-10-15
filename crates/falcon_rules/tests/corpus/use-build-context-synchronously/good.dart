import 'package:flutter/widgets.dart';

Future<void> work() async {}
Object? consume(BuildContext context) => null;

Future<void> guarded(BuildContext context) async {
  await work();
  if (!context.mounted) return;
  context.toString();
}

Future<void> positiveBranch(BuildContext context) async {
  await work();
  if (context.mounted) {
    context.toString();
  }
}

Future<void> guardedBooleanRhs(BuildContext context) async {
  await work();
  if (context.mounted && consume(context) == null) {}
  if (!context.mounted || consume(context) == null) {}
}

Future<void> beforeGap(BuildContext context) async {
  context.toString();
  await work();
}

Future<void> matchingReceiver(BuildContext first, BuildContext second) async {
  await work();
  if (!first.mounted) return;
  first.toString();
  if (second.mounted) {
    second.toString();
  }
}

Future<void> irrefutableGuardFallthrough(BuildContext context, int value) async {
  await work();
  switch (value) {
    case _ when !context.mounted:
      break;
    default:
      context.toString();
  }
}

Future<void> terminatingSwitch(BuildContext context, int value) async {
  switch (value) {
    case 1:
      await work();
      return;
    default:
      break;
  }
  context.toString(); /* expect: use-build-context-synchronously */
}

Future<void> outerGapDoesNotEnterClosure(BuildContext context) async {
  await work();
  final callback = () {
    context.toString();
  };
  callback();
}

Future<void> targetedContinue(BuildContext context, bool keepGoing) async {
  outer: while (keepGoing) {
    if (context.mounted) continue outer;
    break outer;
  }
}

Future<void> forInShadowsContext(BuildContext context) async {
  await work();
  for (int context in <int>[1]) {
    context.toString();
  }
}

Future<void> patternForInShadowsContext(BuildContext context) async {
  await work();
  for (final (int context, _) in <(int, int)>[(1, 2)]) {
    context.toString();
  }
}

Future<void> guardedOuterSurvivesClassicFor(BuildContext context) async {
  await work();
  if (!context.mounted) return;
  for (int context = 0; context < 1; context++) {
    context.toString();
  }
  context.toString();
}

Future<void> classicForMountedCondition(BuildContext context) async {
  await work();
  for (; context.mounted;) {
    context.toString();
    break;
  }
}

Future<void> doWhileMountedExit(BuildContext context) async {
  await work();
  do {} while (!context.mounted);
  context.toString(); /* expect: use-build-context-synchronously */
}

Future<void> guardedGapBranch(BuildContext context, bool flag) async {
  if (flag) {
    await work();
    if (!context.mounted) return;
  }
  context.toString();
}

Future<void> repeatedWhileGuard(BuildContext context, bool keepGoing) async {
  await work();
  while (keepGoing) {
    if (!context.mounted) return;
    context.toString();
  }
}

Future<void> repeatedClassicForGuard(
  BuildContext context,
  bool keepGoing,
) async {
  await work();
  for (; keepGoing;) {
    if (!context.mounted) return;
    context.toString();
  }
}

Future<void> repeatedDoWhileGuard(BuildContext context, bool keepGoing) async {
  await work();
  do {
    if (!context.mounted) return;
    context.toString();
  } while (keepGoing);
}

Future<void> unreachableAfterConditionlessFor(BuildContext context) async {
  for (;;) {
    await work();
  }
  context.toString(); /* expect: use-build-context-synchronously */
}

Future<void> multiDeclaratorShadow(BuildContext context) async {
  await work();
  {
    var context = 0, value = context.toString();
    value.toString();
  }
}

Future<void> unbracedIfScope(
  BuildContext context,
  BuildContext other,
  bool replace,
) async {
  await work();
  if (!context.mounted) return;
  if (replace) { int context = 0; }
  context.toString();
}

Future<void> tryCatchFinallyScopes(
  BuildContext context,
  BuildContext other,
) async {
  await work();
  if (!context.mounted) return;
  try {
    final int context = 0;
    context.toString();
  } on Object catch (context, stackTrace) {
    context.toString();
    stackTrace.toString();
  } finally {
    final int context = 0;
    context.toString();
  }
  context.toString();
}

Future<void> nonContextIfCaseShadow(BuildContext context, Object value) async {
  await work();
  if (value case (int context, _)) {
    context.toString();
  }
}

Future<void> nonContextSwitchShadow(BuildContext context, Object value) async {
  await work();
  switch (value) {
    case (int context, _):
      context.toString();
  }
}

Future<void> unreachableAfterReturningSwitch(
  BuildContext context,
  int value,
) async {
  await work();
  switch (value) {
    case 0:
      return;
    default:
      return;
  }
  context.toString(); /* expect: use-build-context-synchronously */
}

Future<void> returningAssignmentDoesNotLeak(
  BuildContext context,
  bool replace,
) async {
  Object value = 0;
  if (replace) {
    value = context;
    return;
  }
  await work();
  value.toString();
}

Future<void> mutuallyExclusiveAssignment(
  BuildContext context,
  bool useContext,
) async {
  Object value = 0;
  if (useContext) {
    value = context;
  } else {
    await work();
    value.toString();
  }
}

abstract class MyState extends State<StatefulWidget> {
  Future<void> guardedState() async {
    await work();
    if (!mounted) return;
    context.toString();
    this.context.toString();
  }

  Future<void> nonContextLoopShadow() async {
    await work();
    for (int context in <int>[1]) {
      context.toString();
    }
  }

  void localFunctionContextShadow() {
    Future<void> context() async {
      await work();
      context();
    }
    context();
  }

  void nonContextClosureShadow() {
    final callback = (int context) async {
      await work();
      context.toString();
    };
    callback(1);
  }

  void unbracedIfLocalFunctionShadow(bool enabled) {
    if (enabled)
      Future<void> context() async {
        await work();
        context();
      }
  }

  void unbracedLoopLocalFunctionShadow() {
    while (false)
      Future<void> context() async {
        await work();
        context();
      }
  }

  void labeledLocalFunctionShadow() {
    declaration:
    Future<void> context() async {
      await work();
      context();
    }
  }
}

// The official rule matches the mounted getter, not receiver identity.
Future<void> otherContextMountedGetter(
  BuildContext context,
  BuildContext other,
) async {
  await work();
  if (other.mounted) {
    context.toString();
  }
}

Future<void> declaredObjectDoesNotBecomeContext(BuildContext context) async {
  Object value = context;
  await work();
  value.toString();
}

Future<void> declaredDynamicDoesNotBecomeContext(BuildContext context) async {
  dynamic value = 0;
  value = context;
  await work();
  value.toString();
}

Future<void> finallyGuardEscapes(BuildContext context) async {
  try {
    await work();
  } finally {
    if (!context.mounted) return;
  }
  context.toString();
}

Future<void> catchGuardAppliesInsideCatch(BuildContext context) async {
  try {
    await work();
  } catch (_) {
    if (!context.mounted) return;
    context.toString();
  }
}

Future<void> switchMountedWhenGuard(BuildContext context, int value) async {
  await work();
  switch (value) {
    case 0 when context.mounted:
      context.toString();
    default:
      break;
  }
}

Future<void> doBodyGapIsObserved(BuildContext context) async {
  do {
    context.toString(); /* expect: use-build-context-synchronously */
    await work();
  } while (false);
}

abstract class OtherMountedState extends State<StatefulWidget> {
  Future<void> otherStateGetterGuardsStateContext(
    State<StatefulWidget> other,
  ) async {
    await work();
    if (!other.mounted) return;
    context.toString();
  }
}

Future<void> infiniteLoopMountedExit(BuildContext context) async {
  await work();
  if (!context.mounted) {
    while (true) {}
  }
  context.toString();
}

Never stop() => throw StateError('stopped');

Future<void> neverCallMountedExit(BuildContext context) async {
  await work();
  if (!context.mounted) stop();
  context.toString();
}
