import 'package:flutter/widgets.dart';

Future<void> work() async {}
Future<bool> ready() async => true;
void consume(BuildContext context) {}

Future<void> unguarded(BuildContext context) async {
  await work();
  context.toString(); /* expect: use-build-context-synchronously */
}

Future<void> wrongGuard(BuildContext context, BuildContext other) async {
  await work();
  if (!other.mounted) return;
  context.toString(); /* expect: use-build-context-synchronously */
}

Future<void> guardRhs(BuildContext context) async {
  await work();
  if (context.mounted || consume(context) == null) {} /* expect: use-build-context-synchronously */
  if (!context.mounted && consume(context) == null) {} /* expect: use-build-context-synchronously */
}

Future<void> loopBackedge(BuildContext context, bool keepGoing) async {
  while (keepGoing) {
    context.toString(); /* expect: use-build-context-synchronously */
    await work();
  }
}

Future<void> forUpdate(BuildContext context, bool keepGoing) async {
  for (; keepGoing; await work()) {
    context.toString(); /* expect: use-build-context-synchronously */
  }
}

Future<void> tryPath(BuildContext context) async {
  try {
    await work();
  } catch (_) {
    context.toString(); /* expect: use-build-context-synchronously */
  }
}

Future<void> finallyPath(BuildContext context) async {
  try {
    await work();
  } finally {
    context.toString(); /* expect: use-build-context-synchronously */
  }
}

Future<void> switchUnmatched(BuildContext context, int value) async {
  switch (value) {
    case 1:
      await work();
  }
  context.toString(); /* expect: use-build-context-synchronously */
}

Future<void> switchGuardGap(BuildContext context, int value) async {
  switch (value) {
    case 1 when await ready():
      context.toString(); /* expect: use-build-context-synchronously */
  }
}

Future<void> switchGuardFalsePath(BuildContext context, int value) async {
  switch (value) {
    case 1 when await ready():
      break;
    default:
      context.toString(); /* expect: use-build-context-synchronously */
  }
}

Future<void> nestedInternalGap(BuildContext context) async {
  Future<void> local() async {
    await work();
    context.toString(); /* expect: use-build-context-synchronously */
  }
  local();
}

Future<void> classicForDoesNotReplaceOuter(BuildContext context) async {
  await work();
  for (int context = 0; context < 1; context++) {}
  context.toString(); /* expect: use-build-context-synchronously */
}

Future<void> forInContext(List<BuildContext> contexts) async {
  for (final loopContext in contexts) {
    await work();
    loopContext.toString(); /* expect: use-build-context-synchronously */
  }
}

Future<void> patternForInContext(List<(BuildContext, int)> entries) async {
  for (final (loopContext, _) in entries) {
    await work();
    loopContext.toString(); /* expect: use-build-context-synchronously */
  }
}

Future<void> whileBodyDoesNotReplaceOuter(BuildContext context) async {
  await work();
  while (false) int context = 0;
  context.toString(); /* expect: use-build-context-synchronously */
}

Future<void> doWhileBodyDoesNotReplaceOuter(BuildContext context) async {
  await work();
  do int context = 0; while (false);
  context.toString(); /* expect: use-build-context-synchronously */
}

Future<void> closureCapturesForInContext(List<BuildContext> contexts) async {
  for (BuildContext loopContext in contexts) {
    final callback = () async {
      await work();
      loopContext.toString(); /* expect: use-build-context-synchronously */
    };
    callback();
  }
}

Future<void> classicForContextShadow(
  BuildContext context,
  BuildContext other,
) async {
  await work();
  if (!context.mounted || !other.mounted) return;
  for (BuildContext context = other; false;) {
    context.toString(); /* expect: use-build-context-synchronously */
  }
  context.toString();
}

Future<void> forInContextShadow(
  BuildContext context,
  List<BuildContext> contexts,
) async {
  await work();
  if (!context.mounted) return;
  for (BuildContext context in contexts) {
    context.toString(); /* expect: use-build-context-synchronously */
  }
  context.toString();
}

Future<void> patternForInContextShadow(
  BuildContext context,
  List<(BuildContext, int)> entries,
) async {
  await work();
  if (!context.mounted) return;
  for (final (BuildContext context, _) in entries) {
    context.toString(); /* expect: use-build-context-synchronously */
  }
  context.toString();
}

Future<void> whileBreakDoesNotProveMounted(BuildContext context) async {
  await work();
  while (!context.mounted) {
    break;
  }
  context.toString(); /* expect: use-build-context-synchronously */
}

Future<void> doBodyFactDoesNotEscape(
  BuildContext context,
  BuildContext other,
) async {
  await work();
  if (!other.mounted) return;
  do {
    final BuildContext context = other;
    if (!context.mounted) return;
  } while (false);
  context.toString(); /* expect: use-build-context-synchronously */
}

Future<void> conditionalBreakWhile(
  BuildContext context,
  bool keepGoing,
  bool stop,
) async {
  while (keepGoing) {
    await work();
    if (stop) break;
    if (!context.mounted) continue;
  }
  context.toString(); /* expect: use-build-context-synchronously */
}

Future<void> conditionalContinueFor(
  BuildContext context,
  bool keepGoing,
  bool skip,
) async {
  for (; keepGoing;) {
    await work();
    if (skip) continue;
    if (!context.mounted) break;
  }
  context.toString(); /* expect: use-build-context-synchronously */
}

Future<void> conditionalBreakDoWhile(
  BuildContext context,
  bool keepGoing,
  bool stop,
) async {
  do {
    await work();
    if (stop) break;
    if (!context.mounted) continue;
  } while (keepGoing);
  context.toString(); /* expect: use-build-context-synchronously */
}

Future<void> reassignedContext(
  BuildContext context,
  BuildContext other,
) async {
  await work();
  if (!context.mounted || !other.mounted) return;
  context = other;
  context.toString(); /* expect: use-build-context-synchronously */
}

Future<void> inferredAssignmentContext(BuildContext context) async {
  var value = 0;
  value = context;
  await work();
  value.toString(); /* expect: use-build-context-synchronously */
}

Future<void> nestedInferredAssignmentContext(BuildContext context) async {
  var value = 0;
  {
    value = context;
  }
  await work();
  value.toString(); /* expect: use-build-context-synchronously */
}

Future<void> patternReassignedContext(
  BuildContext context,
  BuildContext other,
) async {
  await work();
  if (!context.mounted || !other.mounted) return;
  (context, _) = (other, 0);
  context.toString(); /* expect: use-build-context-synchronously */
}

Future<void> outerTargetedBreak(BuildContext context, bool stop) async {
  outer: while (true) {
    while (true) {
      if (stop) {
        await work();
        break outer;
      }
      return;
    }
  }
  context.toString(); /* expect: use-build-context-synchronously */
}

Future<void> outerTargetedContinue(
  BuildContext context,
  bool keepGoing,
  bool skip,
) async {
  outer: while (keepGoing) {
    context.toString(); /* expect: use-build-context-synchronously */
    while (true) {
      await work();
      if (skip) continue outer;
      return;
    }
  }
}

Future<void> patternDeclarationContext(BuildContext outer) async {
  final (context, _) = (outer, 1);
  await work();
  context.toString(); /* expect: use-build-context-synchronously */
}

Future<void> ifCaseContext(BuildContext outer, Object value) async {
  if (value case (BuildContext context, _)) {
    await work();
    context.toString(); /* expect: use-build-context-synchronously */
  }
}

Future<void> switchPatternContext(BuildContext outer, Object value) async {
  switch (value) {
    case (BuildContext context, _):
      await work();
      context.toString(); /* expect: use-build-context-synchronously */
  }
}

Future<void> dynamicAssignmentPreservesContext(
  BuildContext context,
  dynamic value,
) async {
  context = value;
  await work();
  context.toString(); /* expect: use-build-context-synchronously */
}

Future<void> branchAssignmentKeepsContextPath(
  BuildContext context,
  bool clear,
) async {
  var value = context;
  if (clear) value = null;
  await work();
  value.toString(); /* expect: use-build-context-synchronously */
}

Future<void> mutuallyExclusiveAssignmentStillMerges(
  BuildContext context,
  bool useContext,
) async {
  var value = 0;
  if (useContext) {
    value = context;
  } else {
    value = 0;
  }
  await work();
  value.toString(); /* expect: use-build-context-synchronously */
}

Future<void> defaultBeforeLaterCase(BuildContext context, int value) async {
  switch (value) {
    default:
      break;
    case 1:
      await work();
  }
  context.toString(); /* expect: use-build-context-synchronously */
}

Future<void> defaultBeforeLaterGuard(BuildContext context, int value) async {
  switch (value) {
    default:
      context.toString(); /* expect: use-build-context-synchronously */
      break;
    case 1 when await ready():
      break;
  }
}

Future<void> groupedDefaultBeforeLaterGuard(
  BuildContext context,
  int value,
) async {
  switch (value) {
    default:
    case 1 when await ready():
      context.toString(); /* expect: use-build-context-synchronously */
  }
}

Future<void> chainedSwitchContinue(BuildContext context, int value) async {
  switch (value) {
    first:
    case 0:
      continue second;
    second:
    case 1:
      await work();
      continue third;
    third:
    default:
      context.toString(); /* expect: use-build-context-synchronously */
  }
}

class Holder {
  final BuildContext context;
  Holder(this.context);
}

Future<void> propertyUse(Holder holder) async {
  await work();
  holder.context.toString(); /* expect: use-build-context-synchronously */
}

Future<void> blockPropertyShadow(Holder holder, Holder other) async {
  await work();
  if (!holder.context.mounted) return;
  {
    final Holder holder = other;
    holder.context.toString(); /* expect: use-build-context-synchronously */
  }
  holder.context.toString();
}

Future<void> classicForPropertyShadow(Holder holder, Holder other) async {
  await work();
  if (!holder.context.mounted) return;
  for (Holder holder = other; false;) {
    holder.context.toString(); /* expect: use-build-context-synchronously */
  }
  holder.context.toString();
}

Future<void> forInPropertyShadow(Holder holder, List<Holder> others) async {
  await work();
  if (!holder.context.mounted) return;
  for (final holder in others) {
    holder.context.toString(); /* expect: use-build-context-synchronously */
  }
  holder.context.toString();
}

class MyState extends State<Object> {
  Future<void> stateUses() async {
    await work();
    this.context.toString(); /* expect: use-build-context-synchronously */
    context!.toString(); /* expect: use-build-context-synchronously */
  }

  Future<void> loopContextShadow(List<BuildContext> contexts) async {
    await work();
    if (!this.context.mounted) return;
    for (BuildContext context in contexts) {
      context.toString(); /* expect: use-build-context-synchronously */
    }
  }

  Future<void> parameterContextShadow(BuildContext context) async {
    await work();
    if (!this.context.mounted) return;
    context.toString(); /* expect: use-build-context-synchronously */
  }

  void closureContextShadow() {
    final callback = (BuildContext context) async {
      await work();
      if (!this.context.mounted) return;
      context.toString(); /* expect: use-build-context-synchronously */
    };
    callback(this.context);
  }
}
