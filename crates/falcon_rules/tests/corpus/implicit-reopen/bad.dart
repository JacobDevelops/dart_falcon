interface class I {}

class C extends I {} /* expect: implicit-reopen */

abstract class D extends I {} /* expect: implicit-reopen */

base class B extends I {} /* expect: implicit-reopen */

final class F {}

base class G extends F {} /* expect: implicit-reopen */

mixin M {}

class Alias = I with M; /* expect: implicit-reopen */

base class BaseAlias = F with M; /* expect: implicit-reopen */
