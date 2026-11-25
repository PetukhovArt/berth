use super::blobs::Blobs;
use anyhow::{bail, ensure, Context, Result};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use berth_core::acp::{Attachment, AttachmentUpload, ChatCaps, PromptInput};
use serde_json::{json, Value};
use std::path::{Component, Path, PathBuf};

pub async fn store(
    blobs: &Blobs,
    worktree: &Path,
    uploads: Vec<AttachmentUpload>,
) -> Result<Vec<Attachment>> {
    let mut stored = Vec::with_capacity(uploads.len());
    for upload in uploads {
        stored.push(match upload {
            AttachmentUpload::Image { mime, name, data } => {
                let bytes = STANDARD.decode(data).context("the image is not base64")?;
                Attachment::Image {
                    blob: blobs.put(&bytes).await?,
                    mime,
                    name,
                }
            }
            AttachmentUpload::File { mime, name, text } => Attachment::File {
                blob: blobs.put(text.as_bytes()).await?,
                mime,
                name,
            },
            AttachmentUpload::WorktreeFile { path } => {
                inside(worktree, &path)?;
                Attachment::WorktreeFile { path }
            }
        });
    }
    Ok(stored)
}

pub async fn blocks(
    input: &PromptInput,
    caps: &ChatCaps,
    worktree: &Path,
    blobs: &Blobs,
) -> Result<Vec<Value>> {
    let mut text = input.text.clone();
    let mut attached = Vec::new();
    for attachment in &input.attachments {
        match attachment {
            Attachment::Image { blob, mime, .. } => {
                ensure!(caps.images, "this agent does not take images");
                attached.push(json!({
                    "type": "image",
                    "mimeType": mime,
                    "data": STANDARD.encode(blobs.get(blob).await?),
                }));
            }
            Attachment::File { blob, mime, name } => {
                let body = String::from_utf8(blobs.get(blob).await?)
                    .with_context(|| format!("{name} is not text"))?;
                if caps.embedded_files {
                    attached.push(json!({
                        "type": "resource",
                        "resource": {"uri": file_uri(Path::new(name)), "mimeType": mime, "text": body},
                    }));
                } else {
                    text.push_str(&format!("\n\n{name}:\n```\n{body}\n```"));
                }
            }
            Attachment::WorktreeFile { path } => attached.push(json!({
                "type": "resource_link",
                "uri": file_uri(&inside(worktree, path)?),
                "name": path,
            })),
        }
    }
    let mut blocks = Vec::new();
    if !text.trim().is_empty() {
        blocks.push(json!({"type": "text", "text": text}));
    }
    blocks.extend(attached);
    Ok(blocks)
}

fn inside(worktree: &Path, relative: &str) -> Result<PathBuf> {
    let path = Path::new(relative);
    if path
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        bail!("{relative} is not inside the worktree");
    }
    Ok(worktree.join(path))
}

fn file_uri(path: &Path) -> String {
    let slashed = path.to_string_lossy().replace('\\', "/");
    let mut uri = String::from("file://");
    if !slashed.starts_with('/') {
        uri.push('/');
    }
    for byte in slashed.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' | b':' => {
                uri.push(byte as char)
            }
            _ => uri.push_str(&format!("%{byte:02X}")),
        }
    }
    uri
}
