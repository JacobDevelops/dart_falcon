import 'package:flutter/widgets.dart';

Future<void> portWork() async {}
Future<int> portValue() async => 1;
Future<(int, int)> portRecord() async => (1, 2);
void portUse(BuildContext context) {}
int portRead(BuildContext context) => 0;
List<int> portItems(BuildContext context) => [1];
void portPair(BuildContext context, int value) {}
class PortCapture {
  PortCapture(BuildContext context, int value);
}

// The SDK skips default members when computing switch guard states.
Future<void> defaultMember(BuildContext context, int value) async {
  await portWork();
  switch (value) {
    case 1:
      break;
    default:
      portUse(context);
  }
}

Future<void> onlyDefaultMember(BuildContext context, int value) async {
  await portWork();
  switch (value) {
    default:
      portUse(context);
  }
}

Future<void> defaultBodyGap(BuildContext context, int value) async {
  switch (value) {
    default:
      await portWork();
      portUse(context); /* expect: use-build-context-synchronously */
  }
}

// Analyzer's ForParts ancestor is a barrier; the for visitor then scans
// every evaluation child, including the body, for a reference in the header.
Future<void> headerInitializer(BuildContext context) async {
  for (var i = portRead(context); i < 1; i++) { /* expect: use-build-context-synchronously */
    await portWork();
  }
}

Future<void> headerCondition(BuildContext context) async {
  for (var i = 0; i < portRead(context); i++) { /* expect: use-build-context-synchronously */
    await portWork();
  }
}

Future<void> headerUpdater(BuildContext context) async {
  for (var i = 0; i < 1; i = portRead(context)) { /* expect: use-build-context-synchronously */
    await portWork();
  }
}

Future<void> headerIterable(BuildContext context) async {
  for (final item in portItems(context)) { /* expect: use-build-context-synchronously */
    await portWork();
  }
}

Future<void> collectionHeader(BuildContext context) async {
  final values = [
    for (var i = portRead(context); i < 1; i++) await portValue(), /* expect: use-build-context-synchronously */
  ];
}

Future<void> mapHeader(BuildContext context) async {
  final values = {
    for (var i = portRead(context); i < 1; i++) i: await portValue(), /* expect: use-build-context-synchronously */
  };
}

// ArgumentList is likewise an inert ancestor: the call scans all arguments,
// even an await after the context argument in source order.
Future<void> laterArgument(BuildContext context) async {
  portPair(context, await portValue()); /* expect: use-build-context-synchronously */
}

Future<void> laterConstructorArgument(BuildContext context) async {
  PortCapture(context, await portValue()); /* expect: use-build-context-synchronously */
}

Future<void> laterExplicitConstructorArgument(BuildContext context) async {
  new PortCapture(context, await portValue()); /* expect: use-build-context-synchronously */
}

Future<void> laterFunctionArgument(
  BuildContext context,
  void Function(BuildContext, int) callback,
) async {
  callback(context, await portValue()); /* expect: use-build-context-synchronously */
}

// The official async-state visitor does not override pattern declarations
// or assignments, so these awaits do not establish async-gap states.
Future<void> patternDeclaration(BuildContext context) async {
  final (a, b) = await portRecord();
  portUse(context);
}

Future<void> patternAssignment(BuildContext context) async {
  var a = 0;
  var b = 0;
  (a, b) = await portRecord();
  portUse(context);
}

Future<void> labeledBreakDoesNotExit(BuildContext context) async {
  await portWork();
  if (!context.mounted) {
    outer: while (true) {
      while (true) {
        break outer;
      }
    }
  }
  portUse(context); /* expect: use-build-context-synchronously */
}

// Preserve the SDK ExitDetector's raw switch behavior: each non-empty case
// contains an exiting statement, and the switch has a default member.
Future<void> switchExitDetector(BuildContext context, int value, bool stop) async {
  await portWork();
  if (!context.mounted) {
    switch (value) {
      case 1:
        if (stop) break;
        return;
      default:
        return;
    }
  }
  portUse(context);
}

// Switch guards skip their own arm unless a real expression wrapper causes
// the SDK visitor to re-accept the entire switch, including later arm bodies.
bool portCondition(BuildContext context) => true;
void portAccept(Object? value) {}

Future<void> bareSwitchGuardDeclaration(BuildContext context, int value) async {
  final result = switch (value) {
    1 when portCondition(context) => await portValue(),
    _ => 0,
  };
}

Future<Object?> bareSwitchGuardReturn(BuildContext context, int value) async {
  return switch (value) {
    1 when portCondition(context) => await portValue(),
    _ => 0,
  };
}

// The invocation re-accepts every argument after visiting ArgumentList.
Future<void> bareSwitchGuardArgument(BuildContext context, int value) async {
  portAccept(switch (value) {
    1 when portCondition(context) => await portValue(), /* expect: use-build-context-synchronously */
    _ => 0,
  });
}

Future<void> wrappedSwitchGuard(BuildContext context, int value) async {
  final result = ( /* wrapper */ switch (value) {
    1 when portCondition(context) => await portValue(), /* expect: use-build-context-synchronously */
    _ => 0,
  } /* wrapper */ );
}

Future<void> wrappedSwitchBody(BuildContext context, int value) async {
  portAccept((switch (value) {
    1 => portCondition(context), /* expect: use-build-context-synchronously */
    _ => await portValue(),
  }));
}

Future<void> castSwitchBody(BuildContext context, int value) async {
  final result = switch (value) {
    1 => portCondition(context), /* expect: use-build-context-synchronously */
    _ => await portValue(),
  } as Object;
}

Future<void> genericSdkGetterIsNotContext(List<BuildContext> contexts) async {
  await portWork();
  portUse(contexts.first);
}
