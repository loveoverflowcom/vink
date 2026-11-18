// sqlx::migrate! embeds files at compile time. Adding a migration must also
// invalidate an already-built App, including on stable Rust.
fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
