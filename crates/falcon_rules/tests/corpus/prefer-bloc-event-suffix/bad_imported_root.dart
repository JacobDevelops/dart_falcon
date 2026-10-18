import 'models.dart';
class LocalVariant extends ImportedModel {} /* expect: prefer-bloc-event-suffix */
class Example extends ImportedBase<ImportedModel, int> { Example() : super(0); }
