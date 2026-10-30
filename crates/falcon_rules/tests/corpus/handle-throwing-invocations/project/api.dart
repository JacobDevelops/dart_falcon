import 'package:dart_code_metrics_annotations/annotations.dart';

class ImportedFailure {}
class ImportedChildFailure extends ImportedFailure {}
class ImportedOtherFailure {}
@Throws({ImportedChildFailure})
void importedCall<T>() {}
@Throws()
void importedArbitrary() {}
class ImportedService {
  @Throws({ImportedChildFailure})
  ImportedService();
  @Throws({ImportedChildFailure})
  ImportedService.named();
  @Throws({ImportedChildFailure})
  void run() {}
  @Throws({ImportedChildFailure})
  static void start() {}
}
class DerivedService extends ImportedService {}
