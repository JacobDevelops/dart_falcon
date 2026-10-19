// @dart = 3.13
class PrimaryFailure(final int code) implements Exception {}
class PrimaryError(final String message) extends Error {}
class NotAnException(var int code) {}
