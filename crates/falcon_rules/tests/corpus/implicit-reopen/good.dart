import 'package:meta/meta.dart';

interface class I {}

interface class C extends I {}

final class E extends I {}

sealed class S extends I {}

@reopen
class R extends I {}

final class F {}

final class G extends F {}

sealed class H extends F {}

base class Open {}

base class Plain extends Open {}

// Supertype from another library.
class Remote extends SomeUnknownBase {}

class Base {}

class Derived extends Base {}

sealed class SealedInterface extends I {}

interface class KeepsInterface extends SealedInterface {}

sealed class SealedFinal extends F {}

final class KeepsFinal extends SealedFinal {}

mixin EmptyMixin {}
sealed class SealedAlias = I with EmptyMixin;

interface class KeepsAliasInterface extends SealedAlias {}
