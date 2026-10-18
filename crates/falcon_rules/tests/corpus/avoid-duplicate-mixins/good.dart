import 'one.dart' as one;
import 'two.dart' as two;
mixin M {}
mixin N<T> {}
class A with M, N<int> {}
class B with N<int>, N<String> {}
class C with one.M, two.M {}
class D with M {}
class E with M {}
class F = Object with M, N<double>;
enum G with M { value }
class InterfaceOnly implements M {}
class Applied extends InterfaceOnly with M {}
class GenericBase<T> with N<T> {}
class DifferentArguments extends GenericBase<int> with N<String> {}
class UnknownBase extends missing.Base with M {}
class SeparateBase with M {}
class Separate extends SeparateBase with N<int> {}
mixin Greeter { String greet() => 'hi'; }
mixin OtherGreeter { String greet() => 'other'; }
class BaseGreeter with Greeter {}
class Polite extends BaseGreeter { String greet() => 'good day'; }
class Restored extends Polite with Greeter {}
class OwnOverride with Greeter { String greet() => 'good day'; }
class RestoredOwn extends OwnOverride with Greeter {}
class LaterOverride with Greeter, OtherGreeter {}
class RestoredLater extends LaterOverride with Greeter {}
class Reapplied with Greeter, OtherGreeter, Greeter {}
class UnknownBetween with Greeter, missing.Other, Greeter {}
mixin Delegating on Greeter { String greet() => super.greet(); }
class DelegatingBase extends BaseGreeter with Delegating {}
class DelegatingAgain extends DelegatingBase with Delegating {}
