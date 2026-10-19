void handle(bool a) { /* expect: cognitive-complexity */
  if (a) { try { print(1); } catch (_) { print(2); } }
}
bool choose(bool a, bool b, bool c) { /* expect: cognitive-complexity */
  return a ? (b ? c : false) : false;
}
