void usesChecked(String? checked, String replacement) {
  if (checked != null) { print(checked); print(replacement); }
}
void unrelated(String? checked, int count) {
  if (checked != null) { print(count); }
}
void literal(String? checked) {
  if (checked != null) { print('ready'); }
}
void nullBranch(String? checked, String replacement) {
  if (checked == null) { print(replacement); }
}
void empty(String? checked) { if (checked != null) {} }
void compound(String? checked, String replacement) {
  if (checked != null && checked.isNotEmpty) { print(replacement); }
}
void captured(String? checked, String replacement) {
  if (checked != null) {
    void use() { print(checked); }
    print(replacement);
    use();
  }
}
void elseOnly(String? checked, String replacement) {
  if (checked != null) {} else { print(replacement); }
}
void alreadyNonNull(String checked, String replacement) {
  if (checked != null) { print(replacement); }
}
void branchLocal(String? token, List<String> lines) {
  if (token != null) { for (final line in lines) print(line); }
}
void onlyShadowed(String? checked) {
  if (checked != null) { final checked = 'other'; print(checked); }
}
void callbackParameter(String? checked, List<String> lines) {
  if (checked != null) { lines.forEach((String line) { print(line); }); }
}
void genericMismatch(List<int>? checked, List<String> replacement) {
  if (checked != null) { print(replacement); }
}
