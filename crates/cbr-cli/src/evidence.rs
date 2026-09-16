//! `cbr ingest` and `cbr fetch`, over `evidence/1`.

use std::path::Path;

use cbr_encoding::Value;

use crate::Options;
use crate::session::{Session, at, object, string, subject};

fn now() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs()) as i64;
    let days = seconds.div_euclid(86_400);
    let rem = seconds.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

pub fn ingest(options: &Options, file: &Path) -> Result<(), String> {
    let bytes =
        std::fs::read(file).map_err(|error| format!("reading {}: {error}", file.display()))?;
    let digest = cbr_encoding::digest_bytes(&bytes);
    let hex = digest.split_once(':').map_or(digest.as_str(), |(_, h)| h);
    // Identity is the content plus the moment of ingest: the same bytes
    // ingested twice are two artifacts with their own provenance.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |e| e.as_nanos());
    let artifact = format!("ingest.{}.{nanos}", &hex[..16]);
    // Only the file's name, never its path: a path names a machine.
    let name = file
        .file_name()
        .map_or_else(|| "file".into(), |n| n.to_string_lossy().into_owned());

    // The code a file was captured from: the root tree it belongs to and the
    // commit, both recorded as capture anchors (PROTOCOL-PIN section 5).
    let mut anchors = Vec::new();
    if let Some(repository) = &options.repo {
        let basis = cbr_identity::git_basis(repository, &options.commit)
            .map_err(|error| error.to_string())?;
        anchors.push(object(vec![
            ("kind", string("git_tree")),
            ("id", string(&basis.tree)),
        ]));
        anchors.push(object(vec![
            ("kind", string("git_commit")),
            ("id", string(&basis.commit)),
        ]));
    }

    let mut session = Session::open(
        &options.socket,
        &options.credential_file,
        &["evidence"],
        options.grant.clone(),
    )?;
    let key = subject("evidence.artifact", &artifact);
    let prepared = session.command(
        "evidence.upload.prepare",
        &format!("{artifact}.prepare"),
        key.clone(),
        0,
        None,
        object(vec![
            ("digest", string(&digest)),
            ("size", Value::Int(bytes.len() as i64)),
            ("media_type", string(&options.media_type)),
            ("producer", object(vec![("producer_id", string("cbr-cli"))])),
            (
                "source",
                object(vec![
                    ("kind", string(&options.source_kind)),
                    ("id", string(&name)),
                ]),
            ),
            ("scope", string(&options.scope)),
            (
                "capture",
                object(vec![
                    ("captured_at", string(&now())),
                    ("anchors", Value::Array(anchors)),
                ]),
            ),
            (
                "coverage",
                object(vec![("completeness", string("complete"))]),
            ),
            ("retention_class", string("standard")),
        ]),
    )?;
    let chunk_limit = match at(&prepared, &["outcome", "chunk_limit"]) {
        Value::Int(limit) if limit > 0 => limit as usize,
        _ => return Err("prepare returned no usable chunk_limit".into()),
    };
    let mut revision = 1;
    for (index, chunk) in bytes.chunks(chunk_limit).enumerate() {
        let offset = index * chunk_limit;
        session.command(
            "evidence.upload.append",
            &format!("{artifact}.append.{offset}"),
            key.clone(),
            revision,
            None,
            object(vec![
                ("offset", Value::Int(offset as i64)),
                ("data_base64", string(&cbr_encoding::encode_base64(chunk))),
            ]),
        )?;
        revision += 1;
    }
    let sealed = session.command(
        "evidence.seal",
        &format!("{artifact}.seal"),
        key,
        revision,
        None,
        object(vec![]),
    )?;
    let sealed_digest = at(&sealed, &["outcome", "digest"]);
    if sealed_digest.as_str() != Some(digest.as_str()) {
        return Err(format!(
            "the provider sealed {sealed_digest:?}, not {digest}"
        ));
    }
    println!("artifact {artifact}");
    println!("digest {digest}");
    println!("size {}", bytes.len());
    Ok(())
}

pub fn fetch(options: &Options, artifact: &str, digest: &str, out: &Path) -> Result<(), String> {
    let mut session = Session::open(
        &options.socket,
        &options.credential_file,
        &["evidence"],
        options.grant.clone(),
    )?;
    let mut assembled = Vec::new();
    loop {
        let result = session.query(
            "evidence.fetch",
            object(vec![
                ("artifact", subject("evidence.artifact", artifact)),
                ("digest", string(digest)),
                ("offset", Value::Int(assembled.len() as i64)),
            ]),
        )?;
        let availability = at(&result, &["availability", "state"]);
        if availability.as_str() != Some("available") {
            return Err(format!("the artifact is not available: {availability:?}"));
        }
        let data = result
            .get("data_base64")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let bytes = cbr_encoding::decode_base64(data).ok_or("the provider sent invalid base64")?;
        if bytes.is_empty() {
            return Err("the provider returned no bytes before the end".into());
        }
        assembled.extend(bytes);
        let size = match result.get("size") {
            Some(Value::Int(size)) => *size as usize,
            _ => return Err("fetch returned no size".into()),
        };
        if assembled.len() >= size {
            break;
        }
    }
    // Checked before anything is written: bytes that do not match the sealed
    // digest are never presented as the artifact.
    let computed = cbr_encoding::digest_bytes(&assembled);
    if computed != digest {
        return Err(format!(
            "assembled bytes have digest {computed}, not {digest}; nothing written"
        ));
    }
    let staged = out.with_extension("cbr-partial");
    std::fs::write(&staged, &assembled).map_err(|e| e.to_string())?;
    std::fs::rename(&staged, out).map_err(|e| e.to_string())?;
    println!("wrote {} bytes, digest {digest}", assembled.len());
    Ok(())
}
