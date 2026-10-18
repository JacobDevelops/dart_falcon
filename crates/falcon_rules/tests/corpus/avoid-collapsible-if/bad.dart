void f(bool a, bool b) {
  if (a) { if (b) {} } /* expect: avoid-collapsible-if */
  if (a) if (b) {} /* expect: avoid-collapsible-if */
  if (a || b) { if (!b) return; } /* expect: avoid-collapsible-if */
  if (a) { { if (b) {} } } /* expect: avoid-collapsible-if */
  if (!a) { if (a && b) { print(a); } } /* expect: avoid-collapsible-if */
}
