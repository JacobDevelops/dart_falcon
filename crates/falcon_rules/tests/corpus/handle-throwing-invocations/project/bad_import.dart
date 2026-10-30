import 'package:dart_code_metrics_annotations/annotations.dart';
import 'exported.dart' as api;
import 'lookalike.dart' as custom;

void missing(api.ImportedService service, api.DerivedService derived) {
  api.importedCall<int>(); /* expect: handle-throwing-invocations */
  api.importedArbitrary(); /* expect: handle-throwing-invocations */
  service.run(); /* expect: handle-throwing-invocations */
  derived.run(); /* expect: handle-throwing-invocations */
  api.ImportedService.start(); /* expect: handle-throwing-invocations */
  api.ImportedService(); /* expect: handle-throwing-invocations */
  new api.ImportedService.named(); /* expect: handle-throwing-invocations */
}
@Throws({api.ImportedOtherFailure})
void wrongDeclaration() {
  api.importedCall<int>(); /* expect: handle-throwing-invocations */
}
void wrongCatch() {
  try {
    api.importedCall<int>(); /* expect: handle-throwing-invocations */
  } on api.ImportedOtherFailure {}
}

void customAnnotationMissing() {
  custom.ordinary();
}
void customAnnotationWrongCatch() {
  try {
    custom.ordinary();
  } on api.ImportedFailure {}
}
@custom.Throws({api.ImportedOtherFailure})
void customTypedAnnotationWrongDeclaration() {
  api.importedCall<int>(); /* expect: handle-throwing-invocations */
}

@custom.Throws({api.ImportedFailure})
void customTypedAnnotationDoesNotPropagate() {
  api.importedCall<int>(); /* expect: handle-throwing-invocations */
}

void dependencyDeclarations(DependencyService service) {
  dependencyThrow(); /* expect: handle-throwing-invocations */
  DependencyService(); /* expect: handle-throwing-invocations */
  service.run(); /* expect: handle-throwing-invocations */
}
class ImportedSuper extends api.ImportedService {
  ImportedSuper() : super(); /* expect: handle-throwing-invocations */
}
void importedShorthand() {
  api.ImportedService service = .named(); /* expect: handle-throwing-invocations */
}
