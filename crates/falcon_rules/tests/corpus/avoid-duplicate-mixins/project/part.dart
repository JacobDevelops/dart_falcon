part of 'owner.dart';

class PartBase with SharedMixin {}

class PartDuplicate extends OwnerBase with SharedMixin {} /* expect: avoid-duplicate-mixins */

class InheritedOnly extends OwnerBase {}
