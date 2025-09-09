#[cfg(feature = "cli")]
mod cmd;

#[cfg(feature = "cli")]
fn main() {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(cmd::main());
}

#[cfg(not(feature = "cli"))]
fn main() {}
