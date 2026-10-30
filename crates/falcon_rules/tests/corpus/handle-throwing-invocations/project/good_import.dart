import 'package:dart_code_metrics_annotations/annotations.dart' as metadata;
import 'exported.dart' as api;
import 'lookalike.dart' as unrelated;

@metadata.Throws({api.ImportedFailure})
void declared(api.ImportedService service, api.DerivedService derived) {
  api.importedCall<int>();
  service.run();
  derived.run();
  api.ImportedService.start();
  api.ImportedService();
  new api.ImportedService.named();
}
void caught(api.ImportedService service) {
  try {
    api.importedCall<int>();
    service.run();
    api.ImportedService();
  } on api.ImportedFailure {}
}
void catchAll() {
  try { api.importedArbitrary(); } catch (_) {}
}
@metadata.Throws()
void propagated() { api.importedArbitrary(); }
void customAnnotationCaught() {
  try { unrelated.ordinary(); } catch (_) {}
}
@unrelated.Throws()
void customAnnotationPropagated() { unrelated.ordinary(); }
void shadowed(void Function() importedCall) { importedCall(); }

void dependencyCaught(metadata.DependencyService service) {
  try {
    metadata.dependencyThrow();
    metadata.DependencyService();
    service.run();
  } on metadata.DependencyFailure {}
}
@metadata.Throws({metadata.DependencyFailure})
void dependencyPropagated() { metadata.dependencyThrow(); }
class ImportedSuper extends api.ImportedService {
  @metadata.Throws({api.ImportedFailure})
  ImportedSuper() : super();
}
void importedShorthand() {
  try {
    api.ImportedService service = .named();
  } on api.ImportedFailure {}
}
