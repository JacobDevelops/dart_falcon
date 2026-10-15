// @dart=2.19

abstract class LegacyBehavior {}
class LegacyConsumer extends Object with LegacyBehavior {} /* expect: prefer-mixin */

mixin DeclaredMixin {}
class MixinConsumer extends Object with DeclaredMixin {}
