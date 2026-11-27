//! What a prompt carries besides its typed text: images (validation, the RPC
//! payload, and the decode cache conversation thumbnails draw from) and the
//! reading of a dropped or picked path. The rules that need neither GPUI nor
//! the filesystem (files named by path in the prompt as the tab's backend
//! reads them, the paths a prompt block can't carry, the warnings a refused
//! or gone file gives, the long-paste limits and the `[Pasted text #N]`
//! tokens that stand for them, the `[Image #N]` chips of older messages) live
//! in [`gentle_core::attachments`] and are re-exported here for this
//! feature's callers.

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use gpui_kit::{App, Image, ImageFormat};
use serde_json::{Value, json};

use gentle_core::transcript::{UserImage, image_key};

pub use gentle_core::attachments::{
    AttachedFile, PastedText, Rejection, chip_text, expand, file_name, find_tokens, human_size,
    is_long_paste, missing_files_in, needs_lead, present, rejection_warning_in, remove_token,
    renumber, split_files, summary, token, unprintable, with_files,
};

/// Largest image accepted, before base64.
pub const MAX_IMAGE_BYTES: u64 = 20 * 1024 * 1024;

/// An image waiting in the composer.
#[derive(Clone, Debug)]
pub struct Attachment {
    pub image: Arc<Image>,
    /// The bytes in base64, as sent to Pi.
    pub data: Arc<str>,
    /// [`image_key`] of `data`: the key Pi's echo of this image lands under.
    pub key: u64,
}

impl Attachment {
    /// Accept PNG, JPEG, GIF or WebP bytes (sniffed, not trusted from a
    /// name) up to [`MAX_IMAGE_BYTES`]; BMP and TIFF are converted to PNG
    /// first ([`to_png`]).
    pub fn from_bytes(name: &str, bytes: Vec<u8>) -> Result<Self, Rejection> {
        let bytes = match sniff(&bytes) {
            Some(_) => bytes,
            None => to_png(&bytes).unwrap_or(bytes),
        };
        let size = bytes.len() as u64;
        if size > MAX_IMAGE_BYTES {
            return Err(Rejection::TooLarge {
                name: name.to_string(),
                bytes: size,
            });
        }
        let format = sniff(&bytes).ok_or_else(|| Rejection::Unsupported {
            name: name.to_string(),
        })?;
        let data: Arc<str> = STANDARD.encode(&bytes).into();
        Ok(Self {
            image: Arc::new(Image::from_bytes(format, bytes)),
            key: image_key(&data),
            data,
        })
    }

    pub fn mime_type(&self) -> &'static str {
        self.image.format.mime_type()
    }

    /// Pi's `ImageContent`.
    pub fn payload(&self) -> Value {
        json!({"type": "image", "data": &*self.data, "mimeType": self.mime_type()})
    }
}

/// What a dropped or picked path carried.
#[derive(Debug)]
pub enum Loaded {
    Image(Attachment),
    File(AttachedFile),
}

/// Read a path: an image attaches from its bytes ([`Attachment`]: sniffed,
/// capped at [`MAX_IMAGE_BYTES`]); any other regular file attaches by path
/// ([`AttachedFile`]); a folder is refused ([`Rejection::Folder`]), and so is
/// a path the prompt could not name ([`unprintable`], which judges the line
/// the prompt would carry: `cwd` as `backend` reads it) or one that can't be
/// read ([`Rejection::Unreadable`]). Reading a path is disk work: the tab runs
/// this off the UI thread, when the paths are dropped, pasted or picked.
pub fn load_path(
    path: &Path,
    cwd: &Path,
    backend: Option<&gentle_core::backend::Backend>,
) -> Result<Loaded, Rejection> {
    if let Some(rejection) = unprintable(path, cwd, backend) {
        return Err(rejection);
    }
    let name = file_name(path);
    let unreadable = |error: std::io::Error| Rejection::Unreadable {
        name: name.clone(),
        error: error.to_string(),
    };
    let metadata = std::fs::metadata(path).map_err(unreadable)?;
    if metadata.is_dir() {
        return Err(Rejection::Folder { name });
    }
    if !is_image_file(path).map_err(unreadable)? {
        return Ok(Loaded::File(AttachedFile {
            name,
            path: path.to_path_buf(),
            canonical: canonical(path),
            bytes: metadata.len(),
        }));
    }
    if metadata.len() > MAX_IMAGE_BYTES {
        return Err(Rejection::TooLarge {
            name,
            bytes: metadata.len(),
        });
    }
    let bytes = std::fs::read(path).map_err(unreadable)?;
    Attachment::from_bytes(&name, bytes).map(Loaded::Image)
}

/// `path` with links and `.`/`..` resolved, as far as the disk allows: the
/// spelling two attachments of one file share ([`AttachedFile::canonical`]).
/// Read here, off the UI thread, with the file; a path the disk can't resolve
/// (the file is gone) keeps the spelling it came in with.
fn canonical(path: &Path) -> std::path::PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Whether a path is an image, judged by its first bytes rather than its
/// extension (a misnamed `shot.pnmg` still attaches as an image; a
/// non-image attaches as a file). `Err` when its bytes can't be read.
pub fn is_image_file(path: &Path) -> std::io::Result<bool> {
    use std::io::Read as _;
    let mut head = [0u8; 12];
    let mut file = std::fs::File::open(path)?;
    let mut read = 0;
    while read < head.len() {
        match file.read(&mut head[read..]) {
            Ok(0) => break,
            Ok(n) => read += n,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(is_image_header(&head[..read]))
}

/// The formats [`Attachment::from_bytes`] takes: those [`sniff`] knows, plus
/// BMP (`BM`, then zeroed reserved bytes) and TIFF, which it converts to PNG.
fn is_image_header(head: &[u8]) -> bool {
    sniff(head).is_some()
        || (head.starts_with(b"BM") && head.get(6..10) == Some(&[0; 4]))
        || head.starts_with(b"II*\0")
        || head.starts_with(b"MM\0*")
}

/// A BMP or TIFF re-encoded as PNG, the formats providers don't take but
/// clipboards hand over: Windows' Snipping Tool and Print Screen put a DIB
/// (GPUI gives it as BMP), and some macOS and Linux copies are TIFF. `None`
/// for anything else or bytes that don't decode.
fn to_png(bytes: &[u8]) -> Option<Vec<u8>> {
    use image::codecs::png::{CompressionType, FilterType, PngEncoder};

    let format = image::guess_format(bytes).ok()?;
    if !matches!(format, image::ImageFormat::Bmp | image::ImageFormat::Tiff) {
        return None;
    }
    let decoded = image::load_from_memory_with_format(bytes, format).ok()?;
    let mut png = Vec::new();
    // Fast: a full-screen capture is encoded while the paste waits on it.
    let encoder =
        PngEncoder::new_with_quality(&mut png, CompressionType::Fast, FilterType::Adaptive);
    decoded.write_with_encoder(encoder).ok()?;
    Some(png)
}

/// The image format of `bytes` from its magic number, for the formats Pi's
/// providers accept.
pub fn sniff(bytes: &[u8]) -> Option<ImageFormat> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(ImageFormat::Png)
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(ImageFormat::Jpeg)
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some(ImageFormat::Gif)
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some(ImageFormat::Webp)
    } else {
        None
    }
}

/// RPC `prompt`, steering the running agent when `steer`.
pub fn prompt_command(text: &str, images: &[Attachment], steer: bool) -> Value {
    let mut command = json!({"type": "prompt", "message": text});
    if !images.is_empty() {
        command["images"] = images.iter().map(Attachment::payload).collect();
    }
    if steer {
        command["streamingBehavior"] = "steer".into();
    }
    command
}

// ── Decode cache ───────────────────────────────────────────────────────────

/// Conversation images decoded from base64 once, by [`UserImage::key`].
/// Decoding runs off the UI thread: [`ImageCache::request`] hands out the
/// work and [`ImageCache::finish`] files the results. GPUI keeps its own
/// decoded bitmap of every image drawn until told otherwise, so images
/// this cache forgets are handed back to it ([`ImageCache::release_evicted`]).
#[derive(Default)]
pub struct ImageCache {
    images: HashMap<u64, Slot>,
    /// Forgotten images GPUI may still hold a bitmap of.
    evicted: Vec<Arc<Image>>,
}

enum Slot {
    /// Handed out for decoding; shown as a placeholder meanwhile.
    Decoding,
    /// `None` marks data that is not a displayable image.
    Ready(Option<Arc<Image>>),
}

impl ImageCache {
    /// Seed with a sent attachment so Pi's echo of it needs no decode.
    pub fn insert(&mut self, attachment: &Attachment) {
        self.images
            .insert(attachment.key, Slot::Ready(Some(attachment.image.clone())));
    }

    /// The decoded image; `None` while decoding or when it is not an image.
    pub fn get(&self, image: &UserImage) -> Option<Arc<Image>> {
        match self.images.get(&image.key) {
            Some(Slot::Ready(image)) => image.clone(),
            _ => None,
        }
    }

    /// Mark the shown images not seen yet as decoding and return them to
    /// decode (once per key). With `prune` (`shown` is every image shown),
    /// forget the ones no longer shown.
    pub fn request<'a>(
        &mut self,
        shown: impl IntoIterator<Item = &'a UserImage>,
        prune: bool,
    ) -> Vec<UserImage> {
        let mut keys = HashSet::new();
        let mut jobs = Vec::new();
        for image in shown {
            if prune {
                keys.insert(image.key);
            }
            if let Entry::Vacant(slot) = self.images.entry(image.key) {
                slot.insert(Slot::Decoding);
                jobs.push(image.clone());
            }
        }
        if prune {
            let evicted = &mut self.evicted;
            self.images.retain(|key, slot| {
                let keep = keys.contains(key);
                if let (false, Slot::Ready(Some(image))) = (keep, &*slot) {
                    evicted.push(image.clone());
                }
                keep
            });
        }
        jobs
    }

    /// Drop GPUI's bitmaps of the images forgotten since the last call.
    pub fn release_evicted(&mut self, cx: &mut App) {
        for image in self.evicted.drain(..) {
            image.remove_asset(cx);
        }
    }

    /// Drop GPUI's bitmaps of every image here (the tab is going away).
    pub fn release_all(&mut self, cx: &mut App) {
        self.release_evicted(cx);
        for slot in std::mem::take(&mut self.images).into_values() {
            if let Slot::Ready(Some(image)) = slot {
                image.remove_asset(cx);
            }
        }
    }

    /// File decoded images; ones pruned meanwhile stay forgotten.
    pub fn finish(&mut self, decoded: Vec<(u64, Option<Arc<Image>>)>) {
        for (key, image) in decoded {
            if let Some(slot @ Slot::Decoding) = self.images.get_mut(&key) {
                *slot = Slot::Ready(image);
            }
        }
    }
}

/// Decode `jobs` (from [`ImageCache::request`]); meant for a background thread.
pub fn decode_all(jobs: Vec<UserImage>) -> Vec<(u64, Option<Arc<Image>>)> {
    jobs.iter()
        .map(|image| (image.key, decode(image)))
        .collect()
}

fn decode(image: &UserImage) -> Option<Arc<Image>> {
    let bytes = STANDARD.decode(image.data.as_bytes()).ok()?;
    let format = sniff(&bytes).or_else(|| ImageFormat::from_mime_type(&image.mime_type))?;
    Some(Arc::new(Image::from_bytes(format, bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";
    const JPEG: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, 0, 0x10];

    #[test]
    fn sniffs_supported_formats_only() {
        assert_eq!(sniff(PNG), Some(ImageFormat::Png));
        assert_eq!(sniff(JPEG), Some(ImageFormat::Jpeg));
        assert_eq!(sniff(b"GIF89a\x01\0"), Some(ImageFormat::Gif));
        assert_eq!(sniff(b"GIF87a\x01\0"), Some(ImageFormat::Gif));
        assert_eq!(sniff(b"RIFF\x24\0\0\0WEBPVP8 "), Some(ImageFormat::Webp));
        assert_eq!(sniff(b"RIFF\x24\0\0\0WAVEfmt "), None);
        assert_eq!(sniff(b"BM\x36\0\0\0"), None);
        assert_eq!(sniff(b"<svg xmlns"), None);
        assert_eq!(sniff(b""), None);
    }

    /// Windows' Snipping Tool pastes a DIB, which GPUI hands over as BMP;
    /// providers only take PNG, JPEG, GIF and WebP.
    #[test]
    fn bmp_and_tiff_pastes_are_sent_as_png() {
        let pixels =
            image::RgbImage::from_fn(3, 2, |x, y| image::Rgb([x as u8 * 80, y as u8 * 120, 200]));
        for format in [image::ImageFormat::Bmp, image::ImageFormat::Tiff] {
            let mut bytes = std::io::Cursor::new(Vec::new());
            pixels.write_to(&mut bytes, format).unwrap();
            let attachment =
                Attachment::from_bytes("The pasted image", bytes.into_inner()).unwrap();
            assert_eq!(attachment.mime_type(), "image/png", "{format:?}");
            let png = STANDARD.decode(&*attachment.data).unwrap();
            let back = image::load_from_memory_with_format(&png, image::ImageFormat::Png).unwrap();
            assert_eq!(back.to_rgb8(), pixels, "{format:?} keeps its pixels");
        }
        // Something that only starts like a BMP is still refused.
        assert!(matches!(
            Attachment::from_bytes("x", b"BM\x36\0\0\0junk".to_vec()),
            Err(Rejection::Unsupported { .. })
        ));
    }

    #[test]
    fn validates_type_and_size() {
        let ok = Attachment::from_bytes("a.png", PNG.to_vec()).unwrap();
        assert_eq!(ok.mime_type(), "image/png");
        assert_eq!(&*ok.data, STANDARD.encode(PNG));
        assert_eq!(ok.key, image_key(&ok.data));

        // The name lies; the bytes decide.
        assert_eq!(
            Attachment::from_bytes("fake.png", b"hello".to_vec()).unwrap_err(),
            Rejection::Unsupported {
                name: "fake.png".into()
            }
        );

        let mut big = PNG.to_vec();
        big.resize(MAX_IMAGE_BYTES as usize + 1, 0);
        let error = Attachment::from_bytes("big.png", big).unwrap_err();
        assert!(matches!(error, Rejection::TooLarge { bytes, .. } if bytes == MAX_IMAGE_BYTES + 1));
        assert!(error.to_string().contains("20 MB"));

        let mut limit = PNG.to_vec();
        limit.resize(MAX_IMAGE_BYTES as usize, 0);
        assert!(Attachment::from_bytes("limit.png", limit).is_ok());
    }

    /// A folder of this test's own, outside the repository.
    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("gs-attach-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// S1: a file that is not an image attaches by path instead of being
    /// refused for its format; images keep today's behavior.
    #[test]
    fn regular_files_attach_by_path() {
        let dir = temp_dir("files");
        let notes = dir.join("notes.txt");
        std::fs::write(&notes, "hello").unwrap();
        let pdf = dir.join("report.pdf");
        std::fs::write(&pdf, b"%PDF-1.7\n").unwrap();

        let Loaded::File(file) = load_path(&notes, &dir, None).unwrap() else {
            panic!("notes.txt is a file");
        };
        assert_eq!(file.name, "notes.txt");
        assert_eq!(file.path, notes);
        assert_eq!(file.canonical, std::fs::canonicalize(&notes).unwrap());
        assert_eq!(file.bytes, 5);
        assert_eq!(human_size(file.bytes), "5 B");
        assert!(matches!(
            load_path(&pdf, &dir, None).unwrap(),
            Loaded::File(_)
        ));

        let png = dir.join("shot.PNG");
        std::fs::write(&png, PNG).unwrap();
        let Loaded::Image(image) = load_path(&png, &dir, None).unwrap() else {
            panic!("a PNG is still an image");
        };
        assert_eq!(image.mime_type(), "image/png");

        assert_eq!(human_size(2048), "2.0 KB");
        assert_eq!(human_size(3 * 1024 * 1024), "3.0 MB");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// S2: a folder is still refused, by name.
    #[test]
    fn folders_are_rejected_by_name() {
        let dir = temp_dir("folder");
        let folder = dir.join("shots");
        std::fs::create_dir_all(&folder).unwrap();
        let error = load_path(&folder, &dir, None).unwrap_err();
        assert_eq!(
            error,
            Rejection::Folder {
                name: "shots".into()
            }
        );
        assert_eq!(
            error.to_string(),
            "shots is a folder; attach files instead."
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn loads_files_and_reports_failures() {
        let dir = temp_dir("loads");
        let png = dir.join("shot.PNG");
        std::fs::write(&png, PNG).unwrap();
        let text = dir.join("notes.txt");
        std::fs::write(&text, "hi").unwrap();

        let Loaded::Image(loaded) = load_path(&png, &dir, None).unwrap() else {
            panic!("a PNG is an image");
        };
        assert_eq!(loaded.mime_type(), "image/png");
        assert!(matches!(
            load_path(&text, &dir, None).unwrap(),
            Loaded::File(_)
        ));
        assert!(matches!(
            load_path(&dir.join("missing.png"), &dir, None),
            Err(Rejection::Unreadable { .. })
        ));
        // Judged by content: a misnamed PNG attaches, a `.png` text doesn't.
        let misnamed = dir.join("bug.pnmg");
        std::fs::write(&misnamed, PNG).unwrap();
        let fake = dir.join("fake.png");
        std::fs::write(&fake, "not an image").unwrap();
        assert!(is_image_file(&png).unwrap());
        assert!(is_image_file(&misnamed).unwrap());
        let Loaded::Image(misnamed) = load_path(&misnamed, &dir, None).unwrap() else {
            panic!("a misnamed PNG is still an image");
        };
        assert_eq!(misnamed.mime_type(), "image/png");
        assert!(!is_image_file(&fake).unwrap());
        assert!(!is_image_file(&text).unwrap());
        assert!(is_image_file(&dir.join("missing.png")).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// S11: a path that can't be read is refused as unreadable, never
    /// attached as a file as if it were something else.
    #[cfg(unix)]
    #[test]
    fn an_unreadable_path_is_rejected() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_dir("unreadable");
        let shot = dir.join("shot.png");
        std::fs::write(&shot, PNG).unwrap();
        std::fs::set_permissions(&shot, std::fs::Permissions::from_mode(0o000)).unwrap();
        // Running as root reads it anyway: there is nothing to prove then.
        if std::fs::read(&shot).is_ok() {
            std::fs::remove_dir_all(&dir).unwrap();
            return;
        }
        let rejection = load_path(&shot, &dir, None).unwrap_err();
        assert!(
            matches!(rejection, Rejection::Unreadable { .. }),
            "{rejection:?}"
        );
        assert!(
            rejection
                .to_string()
                .starts_with("Could not read shot.png: ")
        );
        assert!(is_image_file(&shot).is_err());
        std::fs::set_permissions(&shot, std::fs::Permissions::from_mode(0o644)).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// S11, S14/R3-1: a path the prompt's block can't carry (one with a line
    /// break) is refused when it is attached, by name, instead of breaking the
    /// block; a printable file in a folder whose own name has one still
    /// attaches, since the block names the file, not the folder.
    #[cfg(unix)]
    #[test]
    fn paths_with_a_line_break_are_refused() {
        let dir = temp_dir("newline");
        let odd = dir.join("a\nb.txt");
        std::fs::write(&odd, "hi").unwrap();
        assert_eq!(
            load_path(&odd, &dir, None).unwrap_err(),
            Rejection::LineBreak {
                name: "a\nb.txt".to_string()
            }
        );
        assert_eq!(
            load_path(&odd, &dir, None).unwrap_err().to_string(),
            "a\nb.txt has a line break in its path; rename it to attach it."
        );

        let two_lines = dir.join("two\nlines");
        std::fs::create_dir_all(&two_lines).unwrap();
        let inside = two_lines.join("notes.txt");
        std::fs::write(&inside, "hi").unwrap();
        let Loaded::File(file) = load_path(&inside, &two_lines, None).unwrap() else {
            panic!("a printable file in a folder with a line break in its name");
        };
        assert_eq!(file.path, inside);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn serializes_prompt_payload() {
        let image = Attachment::from_bytes("a.png", PNG.to_vec()).unwrap();
        let data = STANDARD.encode(PNG);
        assert_eq!(
            prompt_command("what is this", std::slice::from_ref(&image), false),
            json!({
                "type": "prompt",
                "message": "what is this",
                "images": [{"type": "image", "data": data, "mimeType": "image/png"}],
            })
        );
        assert_eq!(
            prompt_command("again", &[image], true)["streamingBehavior"],
            "steer"
        );
        let plain = prompt_command("hi", &[], false);
        assert_eq!(plain, json!({"type": "prompt", "message": "hi"}));
    }

    #[test]
    fn cache_decodes_once_and_prunes() {
        let attachment = Attachment::from_bytes("a.png", PNG.to_vec()).unwrap();
        let echoed = UserImage::new("image/png", attachment.data.clone());
        let mut cache = ImageCache::default();
        cache.insert(&attachment);
        assert!(Arc::ptr_eq(&cache.get(&echoed).unwrap(), &attachment.image));

        let jpeg = UserImage::new("image/jpeg", STANDARD.encode(JPEG).into());
        let junk = UserImage::new("image/png", "!!not base64".into());
        let jobs = cache.request([&jpeg, &junk, &jpeg], true);
        assert_eq!(jobs.len(), 2);
        // Decoding: a placeholder, and not handed out again.
        assert!(cache.get(&jpeg).is_none());
        assert!(cache.request([&jpeg], false).is_empty());
        // The PNG is no longer shown, so the pruning request dropped it, to
        // be released from GPUI's cache.
        assert!(cache.get(&echoed).is_none());
        assert_eq!(cache.evicted.len(), 1);
        assert!(Arc::ptr_eq(&cache.evicted[0], &attachment.image));

        cache.finish(decode_all(jobs));
        assert_eq!(cache.get(&jpeg).unwrap().format, ImageFormat::Jpeg);
        assert!(cache.get(&junk).is_none());
        assert!(cache.request([&junk], false).is_empty());

        // A conversation with no images forgets them all, though nothing is new.
        assert!(cache.request([], true).is_empty());
        assert!(cache.get(&jpeg).is_none());
        assert_eq!(cache.evicted.len(), 2);
    }

    #[test]
    fn finished_decodes_of_pruned_images_are_dropped() {
        let jpeg = UserImage::new("image/jpeg", STANDARD.encode(JPEG).into());
        let other = UserImage::new("image/png", STANDARD.encode(PNG).into());
        let mut cache = ImageCache::default();
        let jobs = cache.request([&jpeg], true);
        // A new conversation shows only `other` before the decode lands.
        let _ = cache.request([&other], true);
        cache.finish(decode_all(jobs));
        assert!(cache.get(&jpeg).is_none());
        assert_eq!(cache.request([&jpeg], false).len(), 1);
    }
}
