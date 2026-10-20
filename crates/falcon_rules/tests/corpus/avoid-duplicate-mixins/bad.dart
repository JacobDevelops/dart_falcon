mixin M {}
mixin N<T> {}
class A with M, M {} /* expect: avoid-duplicate-mixins */
class B with N<int>, N<int> {} /* expect: avoid-duplicate-mixins */
class C = Object with M, M; /* expect: avoid-duplicate-mixins */
class D with M, M {} /* expect: avoid-duplicate-mixins */
enum E with M, M { value } /* expect: avoid-duplicate-mixins */
mixin class MC {}
class Base with M {}
class Middle extends Base {}
class Inherited extends Middle with M {} /* expect: avoid-duplicate-mixins */
class AliasBase = Object with M;
class InheritedAlias extends AliasBase with M {} /* expect: avoid-duplicate-mixins */
class Direct extends MC with MC {} /* expect: avoid-duplicate-mixins */
class Forward extends Later with M {} /* expect: avoid-duplicate-mixins */
class Later with M {}
mixin Greeter { String greet() => 'hi'; }
mixin Spacer { String other() => 'other'; }
class NoConflict with Greeter, Spacer, Greeter {} /* expect: avoid-duplicate-mixins */
class GreeterBase with Greeter { String other() => 'other'; }
class NoConflictInherited extends GreeterBase with Greeter {} /* expect: avoid-duplicate-mixins */
