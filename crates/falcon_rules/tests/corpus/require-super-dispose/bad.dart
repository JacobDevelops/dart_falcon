import 'package:flutter/material.dart';
class S0 extends State<StatefulWidget> { void dispose() {} /* expect: require-super-dispose */ }
class S1 extends State<StatefulWidget> { void dispose() => cleanup(); /* expect: require-super-dispose */ }
class S2 extends State<StatefulWidget> { void dispose() { cleanup(); } /* expect: require-super-dispose */ }
class S3 extends State<StatefulWidget> { void dispose() { return; } /* expect: require-super-dispose */ }
class S4 extends State<StatefulWidget> { void dispose() { try { cleanup(); } finally { cleanup(); } } /* expect: require-super-dispose */ }
