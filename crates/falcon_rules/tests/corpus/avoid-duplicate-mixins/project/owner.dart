part 'part.dart';

mixin SharedMixin {}

class OwnerBase with SharedMixin {}

class OwnerDuplicate extends PartBase with SharedMixin {} /* expect: avoid-duplicate-mixins */

class FreshApplication with SharedMixin {}
