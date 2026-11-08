//! Generate Python typing declarations from the native binding annotations.

use _native::stub_info;
use std::{env, error::Error, fs, io};

fn main() -> Result<(), Box<dyn Error>> {
    let info = stub_info()?;
    let module = info
        .modules
        .get("osmo_camera._native")
        .ok_or_else(|| io::Error::other("native binding metadata is missing"))?;
    let path = info.python_root.join("osmo_camera").join("_native.pyi");
    let rendered = module.to_string();
    let contents = rendered
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n");
    let contents = format!("{}\n", contents.trim_end());
    match env::args().nth(1).as_deref() {
        None => fs::write(path, contents)?,
        Some("--check") => {
            if fs::read_to_string(path)?.replace("\r\n", "\n") != contents {
                return Err(io::Error::other("native stubs are stale; run stub-gen").into());
            }
        }
        Some(_) => return Err(io::Error::other("usage: stub-gen [--check]").into()),
    }
    Ok(())
}
