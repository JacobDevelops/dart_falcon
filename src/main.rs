#[cfg(all(target_arch = "x86_64", target_os = "linux", target_env = "gnu"))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

fn main() {
    std::process::exit(falcon_cli::run_cli());
}
