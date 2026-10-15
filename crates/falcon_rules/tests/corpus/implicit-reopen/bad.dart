interface class I {}

class C extends I {} /* expect: implicit-reopen */

abstract class D extends I {} /* expect: implicit-reopen */

base class B extends I {} /* expect: implicit-reopen */

final class F {}

base class G extends F {} /* expect: implicit-reopen */

mixin M {}

class Alias = I with M; /* expect: implicit-reopen */

base class BaseAlias = F with M; /* expect: implicit-reopen */

// Sealed intermediates retain restrictions induced by their superclass.
sealed class SealedInterface extends I {}

class ReopensInterface extends SealedInterface {} /* expect: implicit-reopen */

sealed class SealedFinal extends F {}

base class ReopensFinal extends SealedFinal {} /* expect: implicit-reopen */

// A named mixin application is still a class and induces restrictions.
mixin EmptyMixin {}
sealed class SealedAlias = I with EmptyMixin;

class ReopensAlias extends SealedAlias {} /* expect: implicit-reopen */

sealed class SealedFinalAlias = F with EmptyMixin;

base class ReopensFinalAlias extends SealedFinalAlias {} /* expect: implicit-reopen */
