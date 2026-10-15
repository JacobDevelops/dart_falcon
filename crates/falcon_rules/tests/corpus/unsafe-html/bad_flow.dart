class SafePopup {
  void open(String url, String name) {}
}
void popupBranch(dynamic popup, dynamic replacement) {
  popup.open('url', 'name'); /* expect: unsafe-html */
  if (popup is SafePopup) {
    popup.open('url', 'name');
    popup = replacement;
    popup.open('url', 'name'); /* expect: unsafe-html */
  }
  popup.open('url', 'name'); /* expect: unsafe-html */
}
void popupReturn(dynamic popup) {
  if (popup is! SafePopup) return;
  popup.open('url', 'name');
  {
    dynamic popup;
    popup.open('url', 'name'); /* expect: unsafe-html */
  }
  popup.open('url', 'name');
}
void popupCast(dynamic popup) {
  popup as SafePopup;
  popup.open('url', 'name');
}
