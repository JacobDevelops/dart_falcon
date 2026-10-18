import 'package:flutter/material.dart';
class Resource {
 bool dispose() => true;
 void accept(dynamic value) {}
}
class A extends State<StatefulWidget> {
 final a = Resource(); /* expect: dispose-fields */
 final b = Resource(); /* expect: dispose-fields */
 final c = Resource(); /* expect: dispose-fields */
 final d = Resource(); /* expect: dispose-fields */
 final e = Resource(); /* expect: dispose-fields */
 void dispose() {
   a.dispose;
   if (false) b.dispose();
   final callback = () => c.dispose();
   super.dispose();
 }
}

class DeferredCleanup extends State<StatefulWidget> {
 final deferred = Resource(); /* expect: dispose-fields */
 final shortCircuit = Resource(); /* expect: dispose-fields */
 final conditional = Resource(); /* expect: dispose-fields */
 bool enabled = false;
 @override
 void dispose() {
   consume(() => deferred.dispose());
   enabled && shortCircuit.dispose();
   enabled ? conditional.dispose() : false;
   super.dispose();
 }
}

class ResourceHolder {
 final resource = Resource();
 final values = <bool?>[];
 dynamic pending;
}

class OptionalCleanup extends State<StatefulWidget> {
 final coalesced = Resource(); /* expect: dispose-fields */
 final disjunction = Resource(); /* expect: dispose-fields */
 final optionalArgument = Resource(); /* expect: dispose-fields */
 final collectionIf = Resource(); /* expect: dispose-fields */
 final collectionFor = Resource(); /* expect: dispose-fields */
 final mapIf = Resource(); /* expect: dispose-fields */
 final mapFor = Resource(); /* expect: dispose-fields */
 final coalescingAssignment = Resource(); /* expect: dispose-fields */
 final optionalAssignment = Resource(); /* expect: dispose-fields */
 final chainedArgument = Resource(); /* expect: dispose-fields */
 final chainedIndex = Resource(); /* expect: dispose-fields */
 final optionalCascade = Resource(); /* expect: dispose-fields */
 final optionalIndex = Resource(); /* expect: dispose-fields */
 final optionalIndexAssignment = Resource(); /* expect: dispose-fields */
 final cascadeAssignment = Resource(); /* expect: dispose-fields */
 final cascadeIndexAssignment = Resource(); /* expect: dispose-fields */
 final switchArm = Resource(); /* expect: dispose-fields */
 final nullAwareMapKey = Resource(); /* expect: dispose-fields */
 final assertedChain = Resource(); /* expect: dispose-fields */
 String? maybeKey;
 Resource? services;
 ResourceHolder? holder;
 final cascadeHolder = ResourceHolder();
 List<bool?>? values;
 final cascadeValues = <bool?>[null];
 bool? pending;
 bool enabled = false;
 @override
 void dispose() {
   services?.dispose() ?? coalesced.dispose();
   enabled || disjunction.dispose();
   services?.accept(optionalArgument.dispose());
   consume([if (enabled) collectionIf.dispose()]);
   consume([for (final value in <int>[]) collectionFor.dispose()]);
   consume({if (enabled) 'key': mapIf.dispose()});
   consume({for (final value in <int>[]) value: mapFor.dispose()});
   consume({?maybeKey: nullAwareMapKey.dispose()});
   pending ??= coalescingAssignment.dispose();
   holder?.pending = optionalAssignment.dispose();
   holder?.resource.accept(chainedArgument.dispose());
   holder?.resource!.accept(assertedChain.dispose());
   consume(holder?.values[chainedIndex.dispose() ? 0 : 1]);
   services?..accept(optionalCascade.dispose());
   consume(values?[optionalIndex.dispose() ? 0 : 1]);
   values?[0] = optionalIndexAssignment.dispose();
   cascadeHolder..pending ??= cascadeAssignment.dispose();
   cascadeValues..[0] ??= cascadeIndexAssignment.dispose();
   consume(switch (enabled) { _ => switchArm.dispose() });
   super.dispose();
 }
}

class ThrowingArgumentCleanup extends State<StatefulWidget> {
 final afterThrow = Resource(); /* expect: dispose-fields */
 bool enabled = false;
 @override
 void dispose() {
   consumePair(enabled ? throw StateError('stop') : null, afterThrow.dispose());
   super.dispose();
 }
}

class ThrowingCascadeCleanup extends State<StatefulWidget> {
 final resource = Resource(); /* expect: dispose-fields */
 bool enabled = false;
 @override
 void dispose() {
   resource..accept(enabled ? null : throw StateError('stop'))..dispose();
   super.dispose();
 }
}

void consumePair(dynamic first, dynamic second) {}

void consume(dynamic value) {}
