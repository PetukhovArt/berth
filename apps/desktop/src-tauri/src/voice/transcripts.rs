use std::io;
use std::path::PathBuf;
use std::time::Duration;

use berth_core::codec::{read_frame, write_frame};
use berth_core::transport;
use berth_core::{ClientRequest, ServerEvent, PROTOCOL_VERSION};
use tokio::sync::mpsc::unbounded_channel;

use super::Voice;

const RETRY: Duration = Duration::from_secs(5);

pub fn serve(voice: Voice) {
    if !voice.models().supported || crate::ssh_remote::target().is_some() {
        return;
    }
    tauri::async_runtime::spawn(async move {
        loop {
            let _ = session(&voice).await;
            tokio::time::sleep(RETRY).await;
        }
    });
}

async fn session(voice: &Voice) -> io::Result<()> {
    let stream = transport::connect().await?;
    let (reader, mut writer) = tokio::io::split(stream);
    let mut reader = tokio::io::BufReader::new(reader);
    write_frame(
        &mut writer,
        &ClientRequest::Hello {
            protocol_version: PROTOCOL_VERSION,
        },
    )
    .await?;
    match read_frame::<ServerEvent, _>(&mut reader).await? {
        Some(ServerEvent::HelloOk { .. }) => {}
        _ => return Err(io::Error::other("the berth daemon refused the handshake")),
    }
    write_frame(&mut writer, &ClientRequest::OfferTranscription).await?;
    let (answers, mut outbox) = unbounded_channel::<ClientRequest>();
    let writing = async {
        while let Some(request) = outbox.recv().await {
            write_frame(&mut writer, &request).await?;
        }
        Ok::<(), io::Error>(())
    };
    let reading = async {
        while let Some(event) = read_frame::<ServerEvent, _>(&mut reader).await? {
            let ServerEvent::TranscribeVoice { request, pcm_path } = event else {
                continue;
            };
            let voice = voice.clone();
            let answers = answers.clone();
            tauri::async_runtime::spawn(async move {
                let (text, error) = match voice.transcribe_file(PathBuf::from(pcm_path)).await {
                    Ok(text) => (Some(text), None),
                    Err(error) => (None, Some(error)),
                };
                let _ = answers.send(ClientRequest::VoiceTranscribed {
                    request,
                    text,
                    error,
                });
            });
        }
        Ok::<(), io::Error>(())
    };
    tokio::select! {
        written = writing => written,
        read = reading => read,
    }
}
