class Failure implements Exception {
  String message; /* expect: avoid-non-final-exception-class-fields */
  int code = 0; /* expect: avoid-non-final-exception-class-fields */
  int count = 0; /* expect: avoid-non-final-exception-class-fields */
  late Object detail; /* expect: avoid-non-final-exception-class-fields */
  Failure(this.message);
}
class SpecificFailure extends Failure {
  String context = ''; /* expect: avoid-non-final-exception-class-fields */
  SpecificFailure() : super('failure');
}
class FailureError extends Error {
  int code = 0; /* expect: avoid-non-final-exception-class-fields */
}
