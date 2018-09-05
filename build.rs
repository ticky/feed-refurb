const SOURCE_VERSION: &'static str = "SOURCE_VERSION";
const FALLBACK_VERSION: &'static str = "unknown";

/// Ensures we always have a `SOURCE_VERSION` variable, either
/// from the environment, or pulled from git.
/// Falls back to `"unknown"` if no data is available.
fn get_source_version() {
  use std::env;

  let unknown_version = FALLBACK_VERSION.to_string();

  match env::var(SOURCE_VERSION) {
    Err(_) => {
      let parsed_source_version = match std::process::Command::new("git")
        .args(&["rev-parse", "--short", "HEAD"])
        .output()
      {
        Ok(output) => match String::from_utf8(output.stdout) {
          Ok(version) => version,
          Err(_) => unknown_version,
        },

        Err(_) => unknown_version,
      };

      println!(
        "cargo:rustc-env={}={}",
        SOURCE_VERSION, parsed_source_version
      );
    }
    _ => (),
  }
}

fn main() {
  get_source_version();
}
