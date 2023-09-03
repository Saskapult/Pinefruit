use std::path::PathBuf;
use serde::de::DeserializeOwned;


pub fn read_ron<T: DeserializeOwned>(path: impl Into<PathBuf>) -> anyhow::Result<T> {
	let path1 = path.into();
	let path = path1.canonicalize().map_err(|e| anyhow::anyhow!("couldn't find {path1:?} {e:?}"))?;
	let rdr = std::fs::File::open(&path)?;
	let s = ron::de::from_reader::<std::fs::File, T>(rdr)?;
	Ok(s)
}
