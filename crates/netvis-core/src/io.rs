//! `.netvis` project files: pretty-printed JSON with a small versioned envelope.

use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::project::Project;

pub const FILE_EXTENSION: &str = "netvis";
const FORMAT_TAG: &str = "netvis";
const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Error)]
pub enum IoError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("invalid project file: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("not a NetVis project file")]
    WrongFormat,
    #[error("file was saved by a newer NetVis (format v{0}); please upgrade")]
    TooNew(u32),
}

#[derive(Serialize)]
struct EnvelopeOut<'a> {
    format: &'static str,
    version: u32,
    generator: String,
    project: &'a Project,
}

#[derive(Deserialize)]
struct EnvelopeIn {
    format: String,
    version: u32,
    project: Project,
}

pub fn to_string(project: &Project) -> Result<String, IoError> {
    let env = EnvelopeOut {
        format: FORMAT_TAG,
        version: FORMAT_VERSION,
        generator: format!("NetVis {}", env!("CARGO_PKG_VERSION")),
        project,
    };
    Ok(serde_json::to_string_pretty(&env)?)
}

pub fn from_str(s: &str) -> Result<Project, IoError> {
    let env: EnvelopeIn = serde_json::from_str(s)?;
    if env.format != FORMAT_TAG {
        return Err(IoError::WrongFormat);
    }
    if env.version > FORMAT_VERSION {
        return Err(IoError::TooNew(env.version));
    }
    Ok(env.project)
}

/// Write atomically: save to a temp file next to the target, then rename.
pub fn save(project: &Project, path: &Path) -> Result<(), IoError> {
    let text = to_string(project)?;
    let tmp = path.with_extension("netvis.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

pub fn load(path: &Path) -> Result<Project, IoError> {
    from_str(&std::fs::read_to_string(path)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DeviceKind, Endpoint, History, Pos};

    #[test]
    fn roundtrip() {
        let mut p = Project::default();
        let mut h = History::default();
        let (a, e) = p.add_device_edit(DeviceKind::Firewall, Pos::new(1.0, 2.0));
        h.apply(&mut p, e, "");
        let (b, e) = p.add_device_edit(DeviceKind::Internet, Pos::new(5.0, 6.0));
        h.apply(&mut p, e, "");
        let ea = Endpoint { device: a, port: p.first_free_port(a, None).unwrap() };
        let eb = Endpoint { device: b, port: p.first_free_port(b, None).unwrap() };
        let (_, e) = p.add_link_edit(ea, eb).unwrap();
        h.apply(&mut p, e, "");

        let text = to_string(&p).unwrap();
        assert_eq!(from_str(&text).unwrap(), p);
    }

    #[test]
    fn rejects_foreign_and_future_files() {
        let foreign = r#"{"format":"other","version":1,"project":{}}"#;
        assert!(matches!(from_str(foreign), Err(IoError::WrongFormat)));
        let future = r#"{"format":"netvis","version":99,"project":{}}"#;
        assert!(matches!(from_str(future), Err(IoError::TooNew(99))));
    }

    #[test]
    fn missing_fields_take_defaults() {
        let minimal = r#"{"format":"netvis","version":1,"project":{"meta":{"name":"X"}}}"#;
        let p = from_str(minimal).unwrap();
        assert_eq!(p.meta.name, "X");
        assert_eq!(p.meta.company, "Corescale");
        assert!(p.devices.is_empty());
    }

    #[test]
    fn save_and_load_file() {
        let dir = std::env::temp_dir().join(format!("netvis-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.netvis");
        let p = Project::default();
        save(&p, &path).unwrap();
        assert_eq!(load(&path).unwrap(), p);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
