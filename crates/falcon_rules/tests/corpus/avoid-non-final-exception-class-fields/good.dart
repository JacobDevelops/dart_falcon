class Failure implements Exception {
  final String message;
  late final Object detail;
  static int total = 0;
  static const category = 'failure';
  Failure(this.message);
  String get description => message;
}
class SpecificFailure extends Failure {
  final String context;
  SpecificFailure(this.context) : super('failure');
}
class FailureError extends Error { final int code = 1; }
class NotAnException { int mutable = 0; }
