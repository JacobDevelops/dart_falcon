---
default: patch
---

The no-logic-in-create-state rule now reports createState methods that return a constructed type whose fully known inheritance hierarchy does not include State. Types with unresolved ancestors remain conservatively unreported.
