import 'dart:html';
import 'dart:html' as html;

void attributes(
  AnchorElement link,
  IFrameElement frame,
  ScriptElement script,
  EmbedElement embed,
  String url,
) {
  link.href = url; /* expect: unsafe-html */
  frame.src = url; /* expect: unsafe-html */
  frame.srcdoc = '<p>$url</p>'; /* expect: unsafe-html */
  script.src = url; /* expect: unsafe-html */
  embed.src = url; /* expect: unsafe-html */
}

void methods(Element element, DivElement div, Window popup, String markup) {
  element.createFragment(markup); /* expect: unsafe-html */
  div.setInnerHtml(markup); /* expect: unsafe-html */
  popup.open(markup, 'name'); /* expect: unsafe-html */
  window.open(markup, 'name'); /* expect: unsafe-html */
  html.window.open(markup, 'name'); /* expect: unsafe-html */
}

void constructors(String markup) {
  Element.html(markup); /* expect: unsafe-html */
  DocumentFragment.html(markup); /* expect: unsafe-html */
  new html.Element.html(markup); /* expect: unsafe-html */
}

void dynamicReceivers(dynamic target, String url) {
  target.href = url; /* expect: unsafe-html */
  target.src = url; /* expect: unsafe-html */
  target.setInnerHtml(url); /* expect: unsafe-html */
  target.open(url, 'name'); /* expect: unsafe-html */
}

void cascades(String url) {
  AnchorElement()..href = url; /* expect: unsafe-html */
  window..open(url, 'popup'); /* expect: unsafe-html */
}

class SafePopup {
  void open(String url) {}
}

void promotedInOneFunction(dynamic scopedPopup, String url) {
  if (scopedPopup is SafePopup) {
    scopedPopup.open(url);
  }
}

void stillDynamicInAnotherFunction(dynamic scopedPopup, String url) {
  scopedPopup.open(url); /* expect: unsafe-html */
}
