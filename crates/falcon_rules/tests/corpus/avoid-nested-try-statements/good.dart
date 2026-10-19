void handle() {
  try { print(1); } catch (_) {}
  try { void local() { try { print(2); } catch (_) {} } local(); } catch (_) {}
}
