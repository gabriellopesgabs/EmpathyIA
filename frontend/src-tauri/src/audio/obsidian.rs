use std::fs::File;
use std::io::Write;
use std::path::Path;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ObsidianExportData {
    pub title: String,
    pub date: String,
    pub speakers: Vec<String>,
    pub summary_markdown: Option<String>,
    pub transcripts: Vec<ExportTranscriptSegment>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExportTranscriptSegment {
    pub speaker: Option<String>,
    pub text: String,
    pub timestamp: String,
}

/// Generates an Obsidian-compatible Markdown note with YAML frontmatter
pub fn generate_obsidian_markdown(data: &ObsidianExportData) -> String {
    let mut md = String::new();

    // 1. YAML Frontmatter
    md.push_str("---\n");
    md.push_str(&format!("title: {:?}\n", data.title));
    md.push_str(&format!("date: {}\n", data.date));
    
    if !data.speakers.is_empty() {
        md.push_str("speakers:\n");
        for speaker in &data.speakers {
            md.push_str(&format!("  - {:?}\n", speaker));
        }
    }
    
    md.push_str("tags:\n");
    md.push_str("  - meeting\n");
    md.push_str("  - mymeet\n");
    md.push_str("---\n\n");

    // 2. Title Header
    md.push_str(&format!("# {}\n\n", data.title));

    // 3. Summary Section
    md.push_str("## 📝 Summary\n\n");
    if let Some(summary) = &data.summary_markdown {
        md.push_str(summary);
    } else {
        md.push_str("*No summary generated for this meeting.*\n");
    }
    md.push_str("\n\n");

    // 4. Transcript Section
    md.push_str("## 🗣️ Transcript\n\n");
    if data.transcripts.is_empty() {
        md.push_str("*No transcript recording available.*\n");
    } else {
        for segment in &data.transcripts {
            let speaker_name = segment.speaker.as_deref().unwrap_or("Unknown Speaker");
            md.push_str(&format!("**{}** [{}]  \n{}\n\n", speaker_name, segment.timestamp, segment.text));
        }
    }

    md
}

/// Saves the generated Obsidian note to the target vault path
pub fn save_to_obsidian_vault(vault_dir: &str, file_name: &str, markdown_content: &str) -> Result<()> {
    let vault_path = Path::new(vault_dir);
    if !vault_path.exists() {
        return Err(anyhow!("Target Obsidian vault directory does not exist: {}", vault_dir));
    }

    // Clean filename
    let cleaned_filename = format!("{}.md", file_name.replace("/", "_").replace("\\", "_"));
    let dest_file_path = vault_path.join(cleaned_filename);

    let mut file = File::create(&dest_file_path)?;
    file.write_all(markdown_content.as_bytes())?;
    
    log::info!("Successfully exported meeting to Obsidian vault note: {:?}", dest_file_path);
    Ok(())
}

/// Appends a transcript segment in real-time to the Obsidian note.
/// Initializes the file with frontmatter if it does not exist.
pub fn append_transcript_to_obsidian(
    vault_dir: &str,
    meeting_title: &str,
    meeting_date: &str,
    speaker: Option<&str>,
    text: &str,
    timestamp: &str,
) -> Result<()> {
    let vault_path = Path::new(vault_dir);
    if !vault_path.exists() {
        return Err(anyhow!("Target Obsidian vault directory does not exist: {}", vault_dir));
    }

    let cleaned_filename = format!("{}.md", meeting_title.replace("/", "_").replace("\\", "_"));
    let dest_file_path = vault_path.join(cleaned_filename);

    let mut is_new = false;
    if !dest_file_path.exists() {
        is_new = true;
    }

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(true)
        .open(&dest_file_path)?;

    if is_new {
        // Initialize file with frontmatter and headers
        let mut header = String::new();
        header.push_str("---\n");
        header.push_str(&format!("title: {:?}\n", meeting_title));
        header.push_str(&format!("date: {}\n", meeting_date));
        header.push_str("tags:\n");
        header.push_str("  - meeting\n");
        header.push_str("  - mymeet\n");
        header.push_str("---\n\n");
        header.push_str(&format!("# {}\n\n", meeting_title));
        header.push_str("## 📝 Summary\n\n*No summary generated yet.*\n\n");
        header.push_str("## 🗣️ Transcript\n\n");
        file.write_all(header.as_bytes())?;
    }

    let speaker_name = speaker.unwrap_or("Unknown Speaker");
    let line = format!("**{}** [{}]  \n{}\n\n", speaker_name, timestamp, text);
    file.write_all(line.as_bytes())?;

    Ok(())
}

