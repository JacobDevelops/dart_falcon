import 'package:flutter/widgets.dart';

Future<void> work() async {}
void useContext(BuildContext context) {}

class Receiver {
  void show(BuildContext context) {}
}

Future<void> cascadeArgument(BuildContext context, Receiver receiver) async {
  await work();
  receiver..show(context); /* expect: use-build-context-synchronously */
}

Future<void> protectedCascadeCallback(BuildContext context, Future<void> future) async {
  await work();
  future..then((_) {
    useContext(context); /* expect: use-build-context-synchronously */
  });
}

Future<void> nullableLocal(BuildContext? context) async {
  await work();
  useContext(context!); /* expect: use-build-context-synchronously */
}

Future<void> declaredObject(Object context, BuildContext replacement) async {
  context = replacement;
  await work();
  useContext(context as BuildContext);
}

abstract class ContextOwner extends State<StatefulWidget> {
  late BuildContext ownContext;
  BuildContext? nullableContext;

  Future<void> unguardedOwnField() async {
    await work();
    useContext(ownContext); /* expect: use-build-context-synchronously */
  }

  Future<void> stateGetterGuard() async {
    await work();
    if (!mounted) return;
    useContext(ownContext);
  }

  Future<void> nullableField() async {
    await work();
    useContext(nullableContext!);
  }

  Future<void> shadowingParameter(BuildContext ownContext) async {
    await work();
    if (!ownContext.mounted) return;
    useContext(ownContext);
  }

  Future<void> shadowingLocal() async {
    BuildContext ownContext = context;
    await work();
    if (!ownContext.mounted) return;
    useContext(ownContext);
  }

  Future<void> differentReceiverSameGetter(ContextOwner other) async {
    await work();
    if (!other.mounted) return;
    useContext(ownContext);
  }
}

Future<void> inferredRecordBinding(BuildContext outer) async {
  final (context, _) = (outer, 1);
  await work();
  context.toString(); /* expect: use-build-context-synchronously */
}

abstract class StateContextReceiver extends State<StatefulWidget> {
  Future<void> parameterGuard(BuildContext context) async {
    await work();
    if (!this.context.mounted) return;
    context.toString();
  }

  Future<void> loopGuard(List<BuildContext> contexts) async {
    await work();
    if (!this.context.mounted) return;
    for (BuildContext context in contexts) {
      context.toString();
    }
  }

  void closureGuard() {
    final callback = (BuildContext context) async {
      await work();
      if (!this.context.mounted) return;
      context.toString();
    };
    callback(this.context);
  }
}

void switchGuardWithAwaitedArm(
  BuildContext context,
  bool Function(BuildContext) condition,
) async {
  (switch (1) {
    _ when condition(context) => await Future.value(), /* expect: use-build-context-synchronously */
    _ => null,
  });
}

class ContextBox<T> {
  final T value;
  ContextBox(this.value);
}

class DeclaredContextHolder {
  final BuildContext context;
  DeclaredContextHolder(this.context);
  BuildContext get getter => context;
}

Future<void> substitutedContextField(ContextBox<BuildContext> box) async {
  await work();
  useContext(box.value);
}

Future<void> substitutedContextGetter(List<BuildContext> contexts) async {
  await work();
  useContext(contexts.first);
}

Future<void> declaredContextField(DeclaredContextHolder holder) async {
  await work();
  useContext(holder.context); /* expect: use-build-context-synchronously */
}

Future<void> declaredContextGetter(DeclaredContextHolder holder) async {
  await work();
  useContext(holder.getter); /* expect: use-build-context-synchronously */
}
