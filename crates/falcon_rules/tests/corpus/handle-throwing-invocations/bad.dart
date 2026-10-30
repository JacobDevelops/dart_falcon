import 'package:dart_code_metrics_annotations/annotations.dart';
import 'package:unavailable/contracts.dart' show unknownErrors;

class FirstFailure {}
class SecondFailure {}
@Throws({FirstFailure})
void first() {}
@Throws({FirstFailure, SecondFailure})
void both() {}
@Throws()
void arbitrary() {}
@Throws({IndexError})
void indexFailure() {}

void missing() {
  first(); /* expect: handle-throwing-invocations, msg: "Handle this throwing invocation or declare its thrown types with `@Throws`." */
}
void wrongCatch() {
  try {
    first(); /* expect: handle-throwing-invocations */
  } on SecondFailure {}
}
void partialCatch() {
  try {
    both(); /* expect: handle-throwing-invocations */
  } on FirstFailure {}
}
void exceptionDoesNotCatchErrors() {
  try {
    indexFailure(); /* expect: handle-throwing-invocations */
  } on Exception {}
}
void unrestrictedIsNotTyped() {
  try {
    arbitrary(); /* expect: handle-throwing-invocations */
  } on FirstFailure {}
}
@Throws({SecondFailure})
void wrongDeclaration() {
  first(); /* expect: handle-throwing-invocations */
}
@Throws({FirstFailure})
void partialDeclaration() {
  both(); /* expect: handle-throwing-invocations */
}
@Throws({FirstFailure})
void unrestrictedDeclaration() {
  arbitrary(); /* expect: handle-throwing-invocations */
}
void catchBody() {
  try {} catch (_) {
    first(); /* expect: handle-throwing-invocations */
  }
}
void finallyBody() {
  try {} catch (_) {} finally {
    first(); /* expect: handle-throwing-invocations */
  }
}
void deferredClosure() {
  try {
    final action = () {
      first(); /* expect: handle-throwing-invocations */
    };
  } catch (_) {}
}
@Throws()
void nestedFunctionDoesNotInheritAnnotation() {
  void action() {
    first(); /* expect: handle-throwing-invocations */
  }
}
void callbacks(@Throws() void Function() action) {
  action(); /* expect: handle-throwing-invocations */
}
void typedCallback(@Throws({FirstFailure}) void Function() action) {
  try {
    action(); /* expect: handle-throwing-invocations */
  } on SecondFailure {}
}
void localTearOff() {
  @Throws({FirstFailure})
  final action = first;
  action(); /* expect: handle-throwing-invocations */
}
void localFunction() {
  @Throws({FirstFailure})
  void action() {}
  action(); /* expect: handle-throwing-invocations */
}
class Service {
  @Throws({FirstFailure})
  Service();
  @Throws({SecondFailure})
  Service.named();
  @Throws({FirstFailure})
  void run() {}
  @Throws({FirstFailure})
  static void start() {}
}
void methodsAndConstructors(Service service) {
  service.run(); /* expect: handle-throwing-invocations */
  Service.start(); /* expect: handle-throwing-invocations */
  Service(); /* expect: handle-throwing-invocations */
  new Service.named(); /* expect: handle-throwing-invocations */
  service..run(); /* expect: handle-throwing-invocations */
}

void genericCallback(@Throws() void Function<T>() action) {
  action<int>(); /* expect: handle-throwing-invocations */
}

const exceptionTypes = {FirstFailure};
@Throws(exceptionTypes)
void constantContract() {}
void constantContractWrongCatch() {
  try {
    constantContract(); /* expect: handle-throwing-invocations */
  } on SecondFailure {}
}
@Throws({SecondFailure})
void wrongEnclosingDoesNotProveCoverage() {
  first(); /* expect: handle-throwing-invocations */
}

void optionalCallbacks(@Throws() void Function()? callback) {
  callback?.call(); /* expect: handle-throwing-invocations */
  callback!(); /* expect: handle-throwing-invocations */
  callback!.call(); /* expect: handle-throwing-invocations */
}
class CallbackHolder {
  @Throws({FirstFailure})
  void Function()? callback;
}
void fieldCallbacks(CallbackHolder holder) {
  holder.callback?.call(); /* expect: handle-throwing-invocations */
  holder.callback!(); /* expect: handle-throwing-invocations */
}
class ExplicitSuper extends Service {
  ExplicitSuper() : super(); /* expect: handle-throwing-invocations */
  ExplicitSuper.named() : super.named(); /* expect: handle-throwing-invocations */
}
class ImplicitSuper extends Service {
  ImplicitSuper(); /* expect: handle-throwing-invocations */
}
class Redirecting {
  @Throws({FirstFailure})
  Redirecting.target();
  Redirecting() : this.target(); /* expect: handle-throwing-invocations */
}
abstract class RedirectFactory {
  factory RedirectFactory() = FactoryTarget; /* expect: handle-throwing-invocations */
}
class FactoryTarget implements RedirectFactory {
  @Throws({FirstFailure})
  FactoryTarget();
}
void shorthandConstructors() {
  Service service = .named(); /* expect: handle-throwing-invocations */
}
class Primary() {
  @Throws({FirstFailure})
  this { first(); }
}
void primaryConstructor() {
  Primary(); /* expect: handle-throwing-invocations */
}
class UncoveredPrimary() {
  this { first(); /* expect: handle-throwing-invocations */ }
}
class OperatorBoundary {
  @Throws({SecondFailure})
  OperatorBoundary operator +(OperatorBoundary other) {
    first(); /* expect: handle-throwing-invocations */
    return other;
  }
}

class GenericFailure<T> {}
@Throws({GenericFailure<int>})
void genericFailure() {}
void wrongGenericCatch() {
  try {
    genericFailure(); /* expect: handle-throwing-invocations */
  } on GenericFailure<String> {}
}

class ThrowingCallable {
  @Throws({FirstFailure})
  void call() {}
}
void explicitCallableMethod(ThrowingCallable callable) {
  callable.call(); /* expect: handle-throwing-invocations */
}

void directCallbackMissing(List<String> items) {
  items.map((s) => first() /* expect: handle-throwing-invocations */).toList();
}
void directCallbackWrongCatch(List<String> items) {
  try {
    items.map((s) => first() /* expect: handle-throwing-invocations */).toList();
  } on SecondFailure {}
}
@Throws(unknownErrors)
void unknownContract() {}
void unknownContractTypedCatch() {
  try {
    unknownContract(); /* expect: handle-throwing-invocations */
  } on FirstFailure {}
}
