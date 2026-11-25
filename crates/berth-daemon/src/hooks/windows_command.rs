use berth_core::env::{AGENT_ID, API_TOKEN, API_URL};

const TIMEOUT_MS: u32 = super::installer::HOOK_CURL_TIMEOUT_SECS * 1000;

pub fn script(endpoint: &str, event: &str, passthrough: bool) -> String {
    let out = if passthrough {
        "$s = New-Object IO.StreamReader($r.GetResponseStream(), [Text.UTF8Encoding]::new($false)); \
         [Console]::Out.Write($s.ReadToEnd()); "
    } else {
        ""
    };
    format!(
        "$ErrorActionPreference='Stop'; $ProgressPreference='SilentlyContinue'; try {{ \
         if (-not $env:{AGENT_ID} -or -not $env:{API_URL}) {{ exit 0 }}; \
         [Console]::InputEncoding = [Text.UTF8Encoding]::new($false); \
         [Console]::OutputEncoding = [Text.UTF8Encoding]::new($false); \
         $b = [Console]::In.ReadToEnd(); \
         $q = [Net.WebRequest]::Create($env:{API_URL} + '/api/hooks/{endpoint}?agentId=' + $env:{AGENT_ID} + '&hookEvent={event}'); \
         $q.Method = 'POST'; $q.Timeout = {TIMEOUT_MS}; $q.Proxy = $null; \
         $q.ContentType = 'application/json'; \
         $q.Headers.Add('Authorization', 'Bearer ' + $env:{API_TOKEN}); \
         $d = [Text.UTF8Encoding]::new($false).GetBytes($b); \
         $q.ContentLength = $d.Length; \
         $w = $q.GetRequestStream(); $w.Write($d, 0, $d.Length); $w.Close(); \
         $r = $q.GetResponse(); {out}$r.Close() \
         }} catch {{ }}; exit 0"
    )
}

pub fn command(endpoint: &str, event: &str, passthrough: bool) -> String {
    let utf16: Vec<u8> = script(endpoint, event, passthrough)
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    format!(
        "powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -EncodedCommand {}",
        base64(&utf16)
    )
}

fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |acc, (i, b)| acc | (*b as u32) << (16 - 8 * i));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}
