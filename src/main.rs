use {
  anyhow::{anyhow, bail, Context},
  std::{
    collections::HashSet,
    env, fs,
    io::Read,
    path::{Path, PathBuf},
    process::{self, Command, Stdio},
  },
  syn::{Item, UseTree},
  tempdir::TempDir,
  toml_edit::{DocumentMut, Item as TomlItem},
};

type Result<T = (), E = anyhow::Error> = std::result::Result<T, E>;

fn get_cache_dir() -> PathBuf {
  let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
  PathBuf::from(home).join(".run_cache")
}

fn extract_external_dependencies(content: &str) -> Result<HashSet<String>> {
  let syntax = syn::parse_file(content)?;

  let mut dependencies = HashSet::new();

  for item in syntax.items {
    if let Item::Use(item_use) = item {
      extract_dependency_from_use_tree(&item_use.tree, &mut dependencies);
    }
  }

  Ok(dependencies)
}

fn extract_dependency_from_use_tree(
  tree: &UseTree,
  dependencies: &mut HashSet<String>,
) {
  let names = vec!["crate", "self", "super", "std"];

  match tree {
    UseTree::Path(use_path)
      if !names.contains(&use_path.ident.to_string().as_str()) =>
    {
      dependencies.insert(use_path.ident.to_string());
    }
    UseTree::Group(use_group) => {
      for tree in &use_group.items {
        extract_dependency_from_use_tree(tree, dependencies);
      }
    }
    _ => {}
  }
}

fn run_rust(filename: &str) -> Result {
  let temp_dir = TempDir::new(env!("CARGO_PKG_NAME"))?;
  let temp_path = temp_dir.path();

  let cache_dir = get_cache_dir();

  fs::create_dir_all(cache_dir.join("registry"))?;
  fs::create_dir_all(cache_dir.join("target"))?;

  Command::new("cargo")
    .args(&["init", "--bin", "--name", env!("CARGO_PKG_NAME")])
    .current_dir(temp_path)
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .status()
    .context("failed to initialize cargo project")?;

  let src_path = temp_path.join("src").join("main.rs");

  fs::copy(filename, &src_path)?;

  let content = fs::read_to_string(filename)?;

  let dependencies = extract_external_dependencies(&content)
    .context("Failed to extract dependencies")?;

  if !dependencies.is_empty() {
    let cargo_toml_path = temp_path.join("Cargo.toml");

    let mut cargo_toml_content = String::new();

    fs::File::open(&cargo_toml_path)?
      .read_to_string(&mut cargo_toml_content)?;

    let mut doc = cargo_toml_content.parse::<DocumentMut>()?;

    if let Some(TomlItem::Table(deps)) = doc.get_mut("dependencies") {
      for dependency in dependencies {
        deps[&dependency] = toml_edit::value("*");
      }
    } else {
      let mut deps = toml_edit::Table::new();

      for dependency in dependencies {
        deps[&dependency] = toml_edit::value("*");
      }

      doc["dependencies"] = TomlItem::Table(deps);
    }

    fs::write(cargo_toml_path, doc.to_string())?;
  }

  let output = Command::new("cargo")
    .arg("run")
    .arg("--release")
    .env("CARGO_HOME", cache_dir.join("registry"))
    .env("CARGO_TARGET_DIR", cache_dir.join("target"))
    .current_dir(temp_path)
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .output()?;

  if !output.status.success() {
    bail!("failed to build or run rust project");
  }

  println!("{}", String::from_utf8_lossy(&output.stdout).trim());

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

#[cfg(test)]
mod tests {
  use {super::*, indoc::indoc};

  #[test]
  fn extract_single_dependency() {
    let content = indoc! {"
      use rand::Rng;

      fn main() {
        let x: u32 = rand::thread_rng().gen_range(1..=100);
        println!(\"Random number: {}\", x);
      }
    "};

    let dependencies = extract_external_dependencies(content).unwrap();

    assert_eq!(dependencies, HashSet::from(["rand".to_string()]));
  }

  #[test]
  fn extract_multiple_dependencies() {
    let content = indoc! {"
      use rand::Rng;
      use serde_json::Value;
      use reqwest::Client;

      fn main() {
        // Some code using these dependencies
      }
    "};

    let dependencies = extract_external_dependencies(content).unwrap();

    assert_eq!(
      dependencies,
      HashSet::from([
        "rand".to_string(),
        "serde_json".to_string(),
        "reqwest".to_string()
      ])
    );
  }

  #[test]
  fn ignore_std_dependencies() {
    let content = indoc! {"
      use std::collections::HashMap;
      use std::io::Read;

      fn main() {
        // Some code using std
      }
    "};

    let dependencies = extract_external_dependencies(content).unwrap();

    assert!(dependencies.is_empty());
  }

  #[test]
  fn nested_use_statements() {
    let content = indoc! {"
      use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpStream,
      };

      fn main() {
        // Some async code
      }
    "};

    let dependencies = extract_external_dependencies(content).unwrap();

    assert_eq!(dependencies, HashSet::from(["tokio".to_string()]));
  }

  #[test]
  fn mixed_dependencies() {
    let content = indoc! {"
      use std::collections::HashMap;
      use rand::Rng;
      use tokio::io::{AsyncReadExt, AsyncWriteExt};
      use crate::some_module::SomeStruct;

      fn main() {
        // Mixed dependencies
      }
    "};

    let dependencies = extract_external_dependencies(content).unwrap();

    assert_eq!(
      dependencies,
      HashSet::from(["rand".to_string(), "tokio".to_string()])
    );
  }

  #[test]
  fn no_dependencies() {
    let content = indoc! {"
      fn main() {
        println!(\"Hello, world!\");
      }
    "};

    let dependencies = extract_external_dependencies(content).unwrap();

    assert!(dependencies.is_empty());
  }
}
