---
default: minor
---
Implement `prefer-bytes-builder` to recommend efficient byte composition instead of integer-list concatenation, bulk additions, and spreads, reporting each concatenation chain once and avoiding duplicate concatenation reports inside byte spreads.
