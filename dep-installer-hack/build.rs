fn main() {
    // Install external dependency (in the shuttle container only)
    if std::env::var("SHUTTLE").is_ok() {
        if !std::process::Command::new("apt")
            .arg("install")
            .arg("-y")
            .arg("libopus-dev") // the apt package the project needs
            // can add more here
            .status()
            .expect("failed to run apt")
            .success()
        {
            panic!("failed to install dependencies")
        }
    }
}
