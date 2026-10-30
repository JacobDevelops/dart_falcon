import 'package:dart_code_metrics_annotations/annotations.dart' as metadata;
import 'package:unavailable/contracts.dart' show unknownErrors;

class BaseFailure {}
class ChildFailure extends BaseFailure {}
class OtherFailure {}
@metadata.Throws({ChildFailure})
void child() {}
@metadata.Throws({ChildFailure, OtherFailure})
void both() {}
@metadata.Throws()
void arbitrary() {}
void ordinary() {}

void exactCatch() {
  try { child(); } on ChildFailure {}
}
void subtypeCatch() {
  try { child(); } on BaseFailure {}
}
void multipleCatches() {
  try { both(); } on ChildFailure {} on OtherFailure {}
}
void nestedCoverage() {
  try {
    try { both(); } on ChildFailure {}
  } on OtherFailure {}
}
void catchAll() {
  try { arbitrary(); } catch (_) {}
}
void objectCatch() {
  try { arbitrary(); } on Object {}
}
@metadata.Throws({ChildFailure})
void exactDeclaration() { child(); }
@metadata.Throws({BaseFailure})
void coveringDeclaration() { child(); }
@metadata.Throws()
void unrestrictedDeclaration() { arbitrary(); both(); }
@metadata.Throws({OtherFailure})
void combinedDeclarationAndCatch() {
  try { both(); } on BaseFailure {}
}
void nestedCatchBodyCoveredOutside() {
  try {
    try {} catch (_) { child(); }
  } on BaseFailure {}
}
void nestedFinallyCoveredOutside() {
  try {
    try {} finally { child(); }
  } on BaseFailure {}
}
void nestedFunctionHandlesItself() {
  try {
    void action() {
      try { child(); } on BaseFailure {}
    }
  } catch (_) {}
}
void callbackHandled(@metadata.Throws() void Function() action) {
  try { action(); } catch (_) {}
}
@metadata.Throws({BaseFailure})
void callbackPropagated(@metadata.Throws({ChildFailure}) void Function() action) {
  action();
}
void localTearOffHandled() {
  @metadata.Throws({ChildFailure})
  final action = child;
  try { action(); } on BaseFailure {}
}
void localFunctionHandled() {
  @metadata.Throws({ChildFailure})
  void action() {}
  try { action(); } on BaseFailure {}
}
void noContract(void Function() action) { ordinary(); action(); }
void shadowing() {
  final child = ordinary;
  child();
}
void unknown(dynamic receiver) { receiver.invoke(); }
class Service {
  @metadata.Throws({ChildFailure})
  Service();
  @metadata.Throws({ChildFailure})
  Service.named();
  @metadata.Throws({ChildFailure})
  void run() {}
}
void constructorsAndCascades(Service service) {
  try {
    Service();
    new Service.named();
    service.run();
    service..run();
  } on BaseFailure {}
}

void genericCallback(@metadata.Throws() void Function<T>() action) {
  try { action<int>(); } catch (_) {}
}

const exceptionTypes = {ChildFailure};
@metadata.Throws(exceptionTypes)
void constantContract() {}
void constantContractCaught() {
  try { constantContract(); } catch (_) {}
}
@metadata.Throws()
void constantContractPropagated() { constantContract(); }

void optionalCallbacks(@metadata.Throws() void Function()? callback) {
  try {
    callback?.call();
    callback!();
    callback!.call();
  } catch (_) {}
}
class CallbackHolder {
  @metadata.Throws({ChildFailure})
  void Function()? callback;
}
void fieldCallbacks(CallbackHolder holder) {
  try {
    holder.callback?.call();
    holder.callback!();
  } on BaseFailure {}
}
void iterationBindings(List<void Function()> callbacks) {
  @metadata.Throws()
  final runAll = () {
    for (final callback in callbacks) { callback(); }
    final [callback] = callbacks;
    callback();
    try { ordinary(); } catch (caught) { caught(); }
  };
}
void nestedIterationBindings(List<void Function()> callbacks) {
  @metadata.Throws()
  void runAll() {
    callbacks.forEach((ignored) {
      for (final callback in callbacks) { callback(); }
    });
  }
}
class ExplicitSuper extends Service {
  @metadata.Throws({BaseFailure})
  ExplicitSuper() : super();
  @metadata.Throws({BaseFailure})
  ExplicitSuper.named() : super.named();
}
class ImplicitSuper extends Service {
  @metadata.Throws({BaseFailure})
  ImplicitSuper();
}
class Redirecting {
  @metadata.Throws({ChildFailure})
  Redirecting.target();
  @metadata.Throws({BaseFailure})
  Redirecting() : this.target();
}
abstract class RedirectFactory {
  @metadata.Throws({BaseFailure})
  factory RedirectFactory() = FactoryTarget;
}
class FactoryTarget implements RedirectFactory {
  @metadata.Throws({ChildFailure})
  FactoryTarget();
}
void shorthandConstructors() {
  try {
    Service service = .named();
  } on BaseFailure {}
}
class Primary() {
  @metadata.Throws({BaseFailure})
  this { child(); }
}
void primaryConstructor() {
  try { Primary(); } on BaseFailure {}
}
class OperatorBoundary {
  @metadata.Throws({BaseFailure})
  OperatorBoundary operator +(OperatorBoundary other) {
    child();
    return other;
  }
}

class StaticCallName {
  @metadata.Throws()
  StaticCallName();
  static void call() {}
}
void ordinaryStaticCall() {
  StaticCallName.call();
}
class ThrowingParent {
  @metadata.Throws()
  ThrowingParent();
}
class ExternalChild extends ThrowingParent {
  external ExternalChild();
}

void constantTypedCatch() {
  try { constantContract(); } on ChildFailure {}
}
@metadata.Throws(exceptionTypes)
void constantTypedDeclaration() { child(); }
class GenericFailure<T> {}
@metadata.Throws({GenericFailure<int>})
void genericFailure() {}
void genericTypedCatch() {
  try { genericFailure(); } on GenericFailure<int> {}
}
void directCallback(List<String> items) {
  try { items.map((s) => child()).toList(); } on ChildFailure {}
}

class StaticContracts {
  static const errors = {ChildFailure};
  @metadata.Throws(errors)
  void parse() {}
  @metadata.Throws(errors)
  void load() { parse(); }
  void callback(@metadata.Throws(errors) void Function() action) {
    try { action(); } on ChildFailure {}
  }
}
@metadata.Throws(unknownErrors)
void unknownContract() {}
void unknownContractCaught() {
  try { unknownContract(); } catch (_) {}
}
@metadata.Throws()
void unknownContractPropagated() { unknownContract(); }
