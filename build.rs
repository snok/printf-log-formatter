use std::process::Command;

fn main() {
    // Define variables
    let url = "https://raw.githubusercontent.com/charliermarsh/ruff/333f1bd9ceff6390ae652790abb22cd175042bc7/crates/ruff_python_ast/src/visitor.rs";
    let local_path = "src/gen_visitor";

    // Download file
    println!("Downloading {url} to {local_path}");
    Command::new("curl")
        .arg("-fsSL")
        .arg(url)
        .output()
        .expect("Failed to download file")
        .stdout
        .into_iter()
        .for_each(|byte| print!("{}", byte as char));
    println!("File downloaded successfully");
}
