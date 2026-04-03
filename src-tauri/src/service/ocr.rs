use std::path::Path;
use std::process::Command;

pub struct OcrService;

impl OcrService {
    pub fn new() -> Self {
        Self
    }

    #[cfg(target_os = "macos")]
    pub fn extract_text(&self, image_path: &Path) -> Result<String, String> {
        let script = format!(
            r#"
use framework "Vision"
use framework "Foundation"
use scripting additions

set imagePath to "{}"
set theImage to current application's NSImage's alloc()'s initWithContentsOfFile:imagePath

if theImage is missing value then
    return ""
end if

set requestHandler to current application's VNImageRequestHandler's alloc()'s initWithData:(theImage's TIFFRepresentation()) options:(current application's NSDictionary's dictionary())
set theRequest to current application's VNRecognizeTextRequest's alloc()'s init()
theRequest's setRecognitionLevel:(current application's VNRequestTextRecognitionLevelAccurate)

requestHandler's performRequests:{{theRequest}} |error|:(missing value)

set theResults to theRequest's results()
set outputText to ""

repeat with observation in theResults
    set outputText to outputText & (observation's topCandidates:1)'s firstObject()'s |string|() & linefeed
end repeat

return outputText
            "#,
            image_path.to_string_lossy()
        );

        let output = Command::new("osascript")
            .arg("-l")
            .arg("AppleScript")
            .arg("-e")
            .arg(&script)
            .output()
            .map_err(|e| e.to_string())?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }

    #[cfg(target_os = "windows")]
    pub fn extract_text(&self, image_path: &Path) -> Result<String, String> {
        use std::os::windows::process::CommandExt;
        use std::path::PathBuf;
        use std::process::Stdio;
        use std::thread;
        use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let powershell_path =
            PathBuf::from(r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe");

        let script = format!(
            r#"
$utf8 = New-Object System.Text.UTF8Encoding($false)
[Console]::OutputEncoding = $utf8
$OutputEncoding = $utf8

Add-Type -AssemblyName System.Runtime.WindowsRuntime

$imagePath = '{}'

function Write-OcrJson($payload) {{
    Write-Output (ConvertTo-Json $payload -Depth 5 -Compress)
}}

function Write-OcrError([string]$message) {{
    Write-OcrJson @{{
        text = ""
        error = ([string]$message)
    }}
}}

[Windows.Media.Ocr.OcrEngine, Windows.Foundation.UniversalApiContract, ContentType = WindowsRuntime] | Out-Null
[Windows.Graphics.Imaging.BitmapDecoder, Windows.Foundation.UniversalApiContract, ContentType = WindowsRuntime] | Out-Null
[Windows.Storage.StorageFile, Windows.Foundation.UniversalApiContract, ContentType = WindowsRuntime] | Out-Null
[Windows.Globalization.Language, Windows.Foundation.UniversalApiContract, ContentType = WindowsRuntime] | Out-Null

$asTaskGeneric = ([System.WindowsRuntimeSystemExtensions].GetMethods() | Where-Object {{ $_.Name -eq 'AsTask' -and $_.GetParameters().Count -eq 1 -and $_.GetParameters()[0].ParameterType.Name -eq 'IAsyncOperation`1' }})[0]

Function Await($WinRtTask, $ResultType) {{
    if ($null -eq $WinRtTask) {{ throw "Task null" }}
    $asTask = $asTaskGeneric.MakeGenericMethod($ResultType)
    $netTask = $asTask.Invoke($null, @($WinRtTask))
    $netTask.Wait(-1) | Out-Null
    $netTask.Result
}}

try {{
    $file = Await ([Windows.Storage.StorageFile]::GetFileFromPathAsync($imagePath)) ([Windows.Storage.StorageFile])
    $stream = Await ($file.OpenAsync([Windows.Storage.FileAccessMode]::Read)) ([Windows.Storage.Streams.IRandomAccessStream])
    
    $decoder = Await ([Windows.Graphics.Imaging.BitmapDecoder]::CreateAsync($stream)) ([Windows.Graphics.Imaging.BitmapDecoder])
    $bitmap = Await ($decoder.GetSoftwareBitmapAsync()) ([Windows.Graphics.Imaging.SoftwareBitmap])
    
    $ocrBitmap = [Windows.Graphics.Imaging.SoftwareBitmap]::Convert(
        $bitmap,
        [Windows.Graphics.Imaging.BitmapPixelFormat]::Bgra8,
        [Windows.Graphics.Imaging.BitmapAlphaMode]::Premultiplied
    )
    
    $ocrEngine = [Windows.Media.Ocr.OcrEngine]::TryCreateFromUserProfileLanguages()
    if ($ocrEngine -eq $null) {{
        foreach ($langTag in @('zh-Hans', 'zh-Hans-CN', 'en-US')) {{
            try {{
                $language = [Windows.Globalization.Language]::new($langTag)
                $candidate = [Windows.Media.Ocr.OcrEngine]::TryCreateFromLanguage($language)
                if ($candidate -ne $null) {{
                    $ocrEngine = $candidate
                    break
                }}
            }} catch {{}}
        }}
    }}
    
    if ($ocrEngine -eq $null) {{
        Write-OcrError "No OCR engine available"
        exit
    }}
    
    $result = Await ($ocrEngine.RecognizeAsync($ocrBitmap)) ([Windows.Media.Ocr.OcrResult])
    
    $allText = @()
    foreach ($line in $result.Lines) {{
        $allText += $line.Text
    }}
    
    $output = @{{
        text = ($allText -join "`n")
    }}
    
    Write-OcrJson $output
    
    try {{ $stream.Dispose() }} catch {{}}
    try {{ $bitmap.Dispose() }} catch {{}}
    try {{ $ocrBitmap.Dispose() }} catch {{}}
}} catch {{
    Write-OcrError $_.Exception.Message
}}
"#,
            image_path.to_string_lossy().replace("'", "''")
        );

        let script_name = format!(
            "work_review_ocr_{}.ps1",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        let script_path = std::env::temp_dir().join(script_name);

        let bom: &[u8] = b"\xEF\xBB\xBF";
        let script_bytes = script.as_bytes();
        let mut content = Vec::with_capacity(bom.len() + script_bytes.len());
        content.extend_from_slice(bom);
        content.extend_from_slice(script_bytes);

        if let Err(e) = std::fs::write(&script_path, &content) {
            return Err(format!("Failed to write OCR script: {e}"));
        }

        let mut command = Command::new(&powershell_path);
        command
            .args([
                "-NoProfile",
                "-Sta",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
                script_path.to_string_lossy().as_ref(),
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = match command.spawn() {
            Ok(c) => c,
            Err(e) => {
                let _ = std::fs::remove_file(&script_path);
                return Err(format!("Failed to spawn PowerShell: {e}"));
            }
        };

        let started_at = Instant::now();
        let timeout = Duration::from_secs(20);
        let mut output = None;

        loop {
            match child.try_wait() {
                Ok(Some(_)) => {
                    output = child.wait_with_output().ok();
                    break;
                }
                Ok(None) if started_at.elapsed() < timeout => {
                    thread::sleep(Duration::from_millis(100));
                }
                Ok(None) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = std::fs::remove_file(&script_path);
                    return Err("OCR PowerShell script timed out".to_string());
                }
                Err(e) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = std::fs::remove_file(&script_path);
                    return Err(format!("Failed to wait for PowerShell: {e}"));
                }
            }
        }

        let _ = std::fs::remove_file(&script_path);

        if let Some(result) = output {
            if result.status.success() {
                let stdout = String::from_utf8_lossy(&result.stdout);
                if let Ok(ocr_output) = serde_json::from_str::<serde_json::Value>(&stdout) {
                    if let Some(error) = ocr_output.get("error").and_then(|v| v.as_str()) {
                        return Err(format!("OCR Error: {error}"));
                    }
                    let text = ocr_output
                        .get("text")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    return Ok(Self::clean_ocr_text(&text));
                }
                Err("Failed to parse OCR JSON output".to_string())
            } else {
                Err(String::from_utf8_lossy(&result.stderr).to_string())
            }
        } else {
            Err("Failed to get OCR output".to_string())
        }
    }

    #[cfg(target_os = "linux")]
    pub fn extract_text(&self, image_path: &Path) -> Result<String, String> {
        let output = Command::new("tesseract")
            .arg(image_path)
            .arg("stdout")
            .output()
            .map_err(|e| e.to_string())?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }

    /// 清理 OCR 文本（剔除乱码、Markdown 符号等）
    pub fn clean_ocr_text(text: &str) -> String {
        let mut lines: Vec<String> = Vec::new();
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

        for line in text.lines() {
            let cleaned = Self::clean_line(line);
            if cleaned.len() < 2 {
                continue;
            }
            let normalized = cleaned.to_lowercase();
            if seen.contains(&normalized) {
                continue;
            }
            seen.insert(normalized);
            lines.push(cleaned);
        }
        lines.join("\n")
    }

    fn clean_line(line: &str) -> String {
        let mut result = String::new();
        for c in line.chars() {
            if Self::is_valid_char(c) {
                result.push(c);
            } else if c.is_whitespace() {
                if !result.ends_with(' ') && !result.is_empty() {
                    result.push(' ');
                }
            }
        }
        result.trim().to_string()
    }

    fn is_valid_char(c: char) -> bool {
        if ('\u{4e00}'..='\u{9fff}').contains(&c) {
            return true;
        }
        if c.is_ascii_alphabetic() {
            return true;
        }
        if c.is_ascii_digit() {
            return true;
        }
        let punctuation: [char; 30] = [
            '，', '。', '！', '？', '、', '；', '：', '\u{201c}', '\u{201d}', '\u{2018}',
            '\u{2019}', '（', '）', '【', '】', '「', '」', '《', '》', '-', '—', '·', '.', ',',
            ':', ';', '!', '?', '(', ')',
        ];
        if punctuation.contains(&c) {
            return true;
        }
        false
    }
}
