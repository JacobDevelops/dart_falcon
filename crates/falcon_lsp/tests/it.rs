mod cross_file_rules_tests;
mod incremental_tests;
mod lsp_tests;

#[path = "support/atomic_write.rs"]
mod atomic_write;

use atomic_write::write_file_atomically;
