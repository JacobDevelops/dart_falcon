import 'models.dart';
class LocalVariant extends ImportedModel {} /* expect: prefer-bloc-state-suffix */
class Example extends ImportedBase<int, ImportedModel> { Example() : super(ImportedModel()); }
