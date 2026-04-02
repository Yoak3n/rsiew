use std::path::Path;
#[cfg(not(target_os = "windows"))]
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
    pub fn extract_text(&self, _image_path: &Path) -> Result<String, String> {
        Err("Windows OCR not implemented in this snippet".to_string())
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
}