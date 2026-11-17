//! Images for markdown editors: clipboard paste, file drop and the file dialog.
//! Each image becomes an `Upload` action and a placeholder line. `finish_upload` swaps
//! the placeholder for the real link when the upload ends.

use std::io::Cursor;
use std::path::Path;

use eframe::egui::{
    ColorImage, Event, Id, Response, Ui,
    text::{CCursor, CCursorRange},
    text_edit::TextEditState,
};

use super::PanelAction;

const IMAGE_EXTENSIONS: [&str; 5] = ["png", "jpg", "jpeg", "gif", "webp"];
const PH_START: &str = "![Uploading ";
const PH_END: &str = "...]()";

/// The markdown that stands for an upload in progress.
pub fn placeholder(filename: &str) -> String {
    format!("{PH_START}{filename}{PH_END}")
}

/// Inserts pasted or dropped images into `text` at the cursor. Returns true when an
/// upload started. Call it after the `TextEdit` with its response.
pub fn handle(ui: &Ui, response: &Response, text: &mut String, next_request_id: &mut u64, out: &mut Vec<PanelAction>) -> bool {
    let names = take_images(ui, response, next_request_id, out);
    for name in &names {
        insert_at_cursor(ui, response.id, text, &placeholder(name));
    }
    !names.is_empty()
}

/// Starts uploads for images that were pasted into or dropped on this editor.
/// Returns their file names. The caller shows the placeholders.
pub fn take_images(ui: &Ui, response: &Response, next_request_id: &mut u64, out: &mut Vec<PanelAction>) -> Vec<String> {
    // A fixed id, so every editor reads and writes the same slot.
    let last_key = Id::NULL.with("lb-paste-last-editor");
    let focused = response.has_focus();
    if focused {
        ui.ctx().data_mut(|d| d.insert_temp(last_key, response.id));
    }
    let mut files: Vec<(Option<String>, &'static str, Vec<u8>)> = Vec::new();
    if focused {
        let image = ui.input(|i| {
            i.events.iter().find_map(|e| match e {
                Event::PasteImage(img) => Some(img.clone()),
                _ => None,
            })
        });
        // egui sends PasteImage only when the clipboard has no text. A text paste never
        // uploads, so copying text from an app that also puts a bitmap there adds no image.
        let png = image.and_then(|img| encode_color_image(&img));
        if let Some(png) = png {
            files.push((None, "image/png", png));
        }
    }
    // Files dropped on the window go to the focused editor, or else to the last one.
    let last = ui.ctx().data(|d| d.get_temp::<Id>(last_key));
    if focused || last == Some(response.id) {
        let dropped = ui.input(|i| i.raw.dropped_files.clone());
        for f in dropped {
            let path = f.path();
            let (Some(name), Some(ct)) = (file_name(path), content_type(path)) else { continue };
            if let Ok(bytes) = f.bytes() {
                files.push((Some(name), ct, bytes));
            }
        }
    }
    files
        .into_iter()
        .map(|(name, content_type, bytes)| start_upload(name, content_type, bytes, next_request_id, out))
        .collect()
}

/// Opens the file dialog and uploads the picked image. Appends its placeholder to `text`.
/// The dialog blocks the UI thread until it closes.
pub fn attach_dialog(text: &mut String, next_request_id: &mut u64, out: &mut Vec<PanelAction>) -> bool {
    let Some(name) = pick_and_upload(next_request_id, out) else { return false };
    append_line(text, &placeholder(&name));
    true
}

/// Like `attach_dialog`, but returns the file name and leaves the text to the caller.
pub fn pick_and_upload(next_request_id: &mut u64, out: &mut Vec<PanelAction>) -> Option<String> {
    let path = rfd::FileDialog::new()
        .set_title("Attach image")
        .add_filter("Images", &IMAGE_EXTENSIONS)
        .pick_file()?;
    let (name, ct) = (file_name(&path)?, content_type(&path)?);
    let bytes = std::fs::read(&path).ok()?;
    Some(start_upload(Some(name), ct, bytes, next_request_id, out))
}

/// Replaces the placeholder of `filename` with the image link, or removes it when the
/// upload failed or was canceled (`asset_url` is None).
pub fn finish_upload(text: &mut String, _request_id: u64, filename: &str, asset_url: Option<&str>) {
    let ph = placeholder(filename);
    let Some(start) = text.find(&ph) else { return };
    let end = start + ph.len();
    match asset_url {
        Some(url) => text.replace_range(start..end, &format!("![{filename}]({url})")),
        None => {
            // Remove the line break of the placeholder line too, so no blank line stays.
            let (s, e) = if text[end..].starts_with('\n') {
                (start, end + 1)
            } else if text[..start].ends_with('\n') {
                (start - 1, end)
            } else {
                (start, end)
            };
            text.replace_range(s..e, "");
        }
    }
}

/// File names of the uploads that still have a placeholder in `text`, in text order.
pub fn pending_uploads(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find(PH_START) {
        rest = &rest[i + PH_START.len()..];
        let Some(j) = rest.find(PH_END) else { break };
        let name = &rest[..j];
        if !name.is_empty() && !name.contains(['\n', ']', '[']) {
            out.push(name.to_string());
            rest = &rest[j + PH_END.len()..];
        }
    }
    out
}

fn start_upload(
    name: Option<String>,
    content_type: &'static str,
    bytes: Vec<u8>,
    next_request_id: &mut u64,
    out: &mut Vec<PanelAction>,
) -> String {
    let request_id = *next_request_id;
    *next_request_id += 1;
    let filename = name.unwrap_or_else(|| format!("paste-{request_id}.png"));
    out.push(PanelAction::Upload {
        request_id,
        filename: filename.clone(),
        content_type: content_type.to_string(),
        bytes,
    });
    filename
}

/// Inserts `s` as its own line at the editor cursor and moves the cursor after it.
/// Appends on a new line when the editor has no cursor yet.
fn insert_at_cursor(ui: &Ui, id: Id, text: &mut String, s: &str) {
    let ctx = ui.ctx();
    let mut state = TextEditState::load(ctx, id);
    let range = state.as_ref().and_then(|st| st.cursor.char_range());
    let Some(range) = range else {
        append_line(text, s);
        return;
    };
    let char_idx = range.primary.index.0.max(range.secondary.index.0);
    let byte = text.char_indices().nth(char_idx).map_or(text.len(), |(b, _)| b);
    let mut insert = String::new();
    if byte > 0 && !text[..byte].ends_with('\n') {
        insert.push('\n');
    }
    insert.push_str(s);
    if byte < text.len() && !text[byte..].starts_with('\n') {
        insert.push('\n');
    }
    text.insert_str(byte, &insert);
    let after = text[..byte].chars().count() + insert.chars().count();
    if let Some(st) = state.as_mut() {
        st.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(after))));
        st.clone().store(ctx, id);
    }
}

fn append_line(text: &mut String, s: &str) {
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(s);
}

fn file_name(path: &Path) -> Option<String> {
    path.file_name().map(|n| n.to_string_lossy().into_owned())
}

fn content_type(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_string_lossy().to_ascii_lowercase();
    match ext.as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}

fn encode_color_image(img: &ColorImage) -> Option<Vec<u8>> {
    let rgba = img.pixels.iter().flat_map(|c| c.to_srgba_unmultiplied()).collect();
    encode_rgba(img.size[0] as u32, img.size[1] as u32, rgba)
}

fn encode_rgba(width: u32, height: u32, rgba: Vec<u8>) -> Option<Vec<u8>> {
    let img = image::RgbaImage::from_raw(width, height, rgba)?;
    let mut buf = Vec::new();
    img.write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Png).ok()?;
    Some(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finish_upload_puts_the_link_in() {
        let mut s = "Fix:\n![Uploading paste-3.png...]()\nDone".to_string();
        finish_upload(&mut s, 3, "paste-3.png", Some("https://x/a.png"));
        assert_eq!(s, "Fix:\n![paste-3.png](https://x/a.png)\nDone");
    }

    #[test]
    fn finish_upload_without_url_removes_the_line() {
        let mut s = "Fix:\n![Uploading a.png...]()\nDone".to_string();
        finish_upload(&mut s, 1, "a.png", None);
        assert_eq!(s, "Fix:\nDone");

        let mut s = "Fix:\n![Uploading a.png...]()".to_string();
        finish_upload(&mut s, 1, "a.png", None);
        assert_eq!(s, "Fix:");

        let mut s = "![Uploading a.png...]()".to_string();
        finish_upload(&mut s, 1, "a.png", None);
        assert_eq!(s, "");
    }

    #[test]
    fn finish_upload_changes_one_placeholder_and_ignores_others() {
        let mut s = "![Uploading a.png...]()\n![Uploading a.png...]()".to_string();
        finish_upload(&mut s, 1, "a.png", Some("u1"));
        assert_eq!(s, "![a.png](u1)\n![Uploading a.png...]()");
        let before = s.clone();
        finish_upload(&mut s, 2, "b.png", Some("u2"));
        assert_eq!(s, before);
    }

    #[test]
    fn pending_uploads_lists_names_in_order() {
        let s = "Hi\n![Uploading paste-1.png...]()\n![done.png](https://x)\n![Uploading shot 2.jpg...]()";
        assert_eq!(pending_uploads(s), vec!["paste-1.png", "shot 2.jpg"]);
        assert!(pending_uploads("![Uploading ...]() and ![x](y)").is_empty());
        assert!(pending_uploads("no images").is_empty());
    }

    #[test]
    fn content_types_by_extension() {
        assert_eq!(content_type(Path::new("/a/B.JPG")), Some("image/jpeg"));
        assert_eq!(content_type(Path::new("x.webp")), Some("image/webp"));
        assert_eq!(content_type(Path::new("x.txt")), None);
    }

    #[test]
    fn encodes_png() {
        let png = encode_rgba(2, 1, vec![255, 0, 0, 255, 0, 255, 0, 128]).unwrap();
        assert!(png.starts_with(&[0x89, b'P', b'N', b'G']));
        assert!(encode_rgba(3, 3, vec![0; 4]).is_none());
    }
}
