import 'dart:html';

class Link {
  String href = '';
  String src = '';
  void open(String url) {}
  void setInnerHtml(String html) {}
}

void safe(Link link, AnchorElement anchor, Element element, String url) {
  link.href = url;
  link.src = url;
  link.open(url);
  link.setInnerHtml(url);
  anchor.text = url;
  element.innerText = url;
  element.appendText(url);
}

void unknownReceivers(Object target, String url) {
  final items = <String>[url];
  items.toString();
  print(target);
}

void untypedLocal(String url) {
  var href = '';
  href = url;
  print(href);
}

class Foo {
  void open(String url) {}
}

void promotedDynamic(dynamic w, String url) {
  if (w is Foo) {
    w.open(url);
  }
}
