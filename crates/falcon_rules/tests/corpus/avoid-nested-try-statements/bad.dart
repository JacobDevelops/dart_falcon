void handle() {
  try {
    try { print(1); } catch (_) {} /* expect: avoid-nested-try-statements */
  } catch (_) {}
}
