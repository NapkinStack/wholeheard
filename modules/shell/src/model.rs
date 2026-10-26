//! The model a transcript is heard with, fetched on the user's word: its size and its host are
//! shown first; it is written beside where it goes, under `.part`, taken up again where a cut
//! left it, and put in its place only once its whole content has the published fingerprint.

use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

use sha2::{Digest, Sha256};

/// A model as its publisher gives it: where, how big, and its fingerprint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Published {
    pub address: &'static str,
    /// The host the user is shown and agrees to.
    pub host: &'static str,
    pub bytes: u64,
    /// SHA-256 of the whole file, as the publisher states it.
    pub sha256: &'static str,
}

/// The default model, `ggml-large-v3-turbo-q8_0.bin` (ADR-0003), as whisper.cpp's authors
/// publish it on Hugging Face.
pub const DEFAULT: Published = Published {
    address: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q8_0.bin",
    host: "huggingface.co",
    bytes: 874_188_075,
    sha256: "317eb69c11673c9de1e1f0d459b253999804ec71ac4c23c17ecf5fbe24e259a1",
};

/// The file a published model is kept as: its address's last part.
pub fn file(published: &Published) -> &'static str {
    published.address.rsplit('/').next().unwrap_or(published.address)
}

/// The size as a person reads it: megabytes under a gigabyte, then gigabytes, decimal.
pub fn size(bytes: u64) -> String {
    if bytes < 1_000_000_000 {
        format!("{} MB", (bytes as f64 / 1e6).round())
    } else {
        format!("{:.1} GB", bytes as f64 / 1e9)
    }
}

/// Where a fetch writes until the file is whole and checked.
pub fn part(to: &Path) -> PathBuf {
    let mut name = to.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    to.with_file_name(name)
}

/// Fetches a published model to `to`, from `address` (the published one, or a test's). What a
/// cut left in `.part` is taken up again where it stopped; `progress` is told the bytes held
/// so far. The model is put in its place only whole and with its fingerprint: a file that is
/// not the published one is removed, and the next fetch starts again from nothing.
pub fn fetch(published: &Published, address: &str, to: &Path, progress: &mut dyn FnMut(u64)) -> Result<(), String> {
    let part = part(to);
    if let Some(folder) = to.parent() {
        fs::create_dir_all(folder).map_err(|error| format!("{} cannot be made ({error})", folder.display()))?;
    }
    let mut held = fs::metadata(&part).map_or(0, |part| part.len());
    if held > published.bytes {
        held = 0;
    }
    if held < published.bytes {
        held = receive(address, &part, held, progress)?;
    }
    progress(held);
    let whole = held == published.bytes && fingerprint(&part).is_ok_and(|found| found == published.sha256);
    if !whole {
        let _ = fs::remove_file(&part);
        return Err(format!(
            "what came from {} is not the published model, and was removed: it is fetched again from the start next time",
            published.host
        ));
    }
    fs::rename(&part, to).map_err(|error| format!("{} cannot be put in its place ({error})", to.display()))
}

/// Receives the file from `held` bytes on into `part`, and says how many it holds then. A
/// server that does not take up a file where it stopped sends it whole: it is written anew.
fn receive(address: &str, part: &Path, held: u64, progress: &mut dyn FnMut(u64)) -> Result<u64, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(30)))
        .timeout_recv_response(Some(Duration::from_secs(60)))
        .http_status_as_error(false)
        .build()
        .into();
    let mut asked = agent.get(address);
    if held > 0 {
        asked = asked.header("Range", format!("bytes={held}-"));
    }
    let mut answer = asked.call().map_err(|error| format!("the model could not be fetched ({error})"))?;
    let (mut file, mut held) = match answer.status().as_u16() {
        206 if held > 0 => (OpenOptions::new().append(true).open(part), held),
        200 => (File::create(part), 0),
        // What was held is not a beginning of the file the server has: started again next time.
        416 => {
            let _ = fs::remove_file(part);
            return Err("the server no longer has what was begun: the model is fetched again from the start next time".into());
        }
        status => return Err(format!("the model could not be fetched (HTTP {status})")),
    };
    let file = file
        .as_mut()
        .map_err(|error| format!("{} cannot be written ({error})", part.display()))?;
    let mut body = answer.body_mut().as_reader();
    let mut buffer = vec![0; 1 << 16];
    loop {
        // A cut keeps what was written: the next fetch takes it up from there.
        let read = match body.read(&mut buffer) {
            Ok(0) => return Ok(held),
            Ok(read) => read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(format!("the fetch was cut ({error}): it goes on from there next time")),
        };
        file.write_all(&buffer[..read])
            .map_err(|error| format!("{} cannot be written ({error})", part.display()))?;
        held += read as u64;
        progress(held);
    }
}

/// The SHA-256 of a file, in lower-case hexadecimal.
fn fingerprint(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    io::copy(&mut file, &mut hasher)?;
    Ok(hasher.finalize().iter().map(|byte| format!("{byte:02x}")).collect())
}
