#[cfg(feature = "cli")]
#[tokio::main]
async fn main() {
    println!("Hello, world!");
}

#[cfg(not(feature = "cli"))]
fn main() {}
