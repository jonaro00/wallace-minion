fn main() {
    println!("cargo:rerun-if-changed=prisma/schema.prisma");
    if !std::process::Command::new("cargo")
        .args([
            "run",
            "-p",
            "prisma-cli",
            "--target-dir",
            "target_prisma-cli",
            "--",
            "generate",
        ])
        .status()
        .expect("failed to build prisma")
        .success()
    {
        panic!("failed to generate prisma")
    }
}
