fn main() {
    println!("cargo:rerun-if-changed=prisma/schema.prisma");
    std::process::Command::new("cargo")
        .args(["run", "-p", "prisma-cli", "--target-dir", "target_prisma-cli", "--", "generate"])
        .spawn()
        .expect("failed to build prisma");
}
