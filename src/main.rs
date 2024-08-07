use anyhow::{anyhow, bail};
use std::env;
use std::path::Path;
use std::process;
use std::process::Command;
use tempdir::TempDir;

type Result<T = (), E = anyhow::Error> = std::result::Result<T, E>;

fn run_rust(filename: &str) -> Result {
  let temp_dir = TempDir::new("run")?;

  let output_path = temp_dir
    .path()
    .join(Path::new(filename).file_stem().unwrap().to_str().unwrap());

  let status = Command::new("rustc")
    .arg(filename)
    .arg("-o")
    .arg(&output_path)
    .status()?;

  if !status.success() {
    bail!("failed to compile rust file");
  }

  let status = Command::new(&output_path).status()?;

  if !status.success() {
    bail!("failed to execute rust binary");
  }

  Ok(())
}

fn run_python(filename: &str) -> Result {
  let status = Command::new("python").arg(filename).status()?;

  if !status.success() {
    bail!("failed to execute python script");
  }

  Ok(())
}

fn run() -> Result {
  let args: Vec<String> = env::args().collect();

  if args.len() != 2 {
    eprintln!("usage: {} <filename>", args[0]);
    process::exit(1);
  }

  let filename = &args[1];

  let extension = Path::new(filename)
    .extension()
    .and_then(|ext| ext.to_str())
    .ok_or(anyhow!("Failed to get file extension"))?;

  match extension {
    "rs" => run_rust(filename)?,
    "py" => run_python(filename)?,
    _ => bail!("unsupported file type: {}", extension),
  }

  Ok(())
}

fn main() {
  if let Err(error) = run() {
    eprintln!("error: {}", error);
    process::exit(1);
  }
}
