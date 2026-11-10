use anyhow::{ensure, Context, Result};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
};

const EDIT_LIMIT: u64 = 96 * 1024;

pub struct Draft {
    original: Vec<u8>,
    initial: String,
    pub text: String,
}

impl Draft {
    pub fn load(path: &Path) -> Result<Self> {
        let original = read_small(path)?;
        let text = std::str::from_utf8(&original).context("Inline editing requires UTF-8 text")?;
        let initial = text.strip_prefix('\u{feff}').unwrap_or(text).to_owned();
        ensure!(
            !initial
                .chars()
                .any(|ch| ch.is_control() && !matches!(ch, '\t' | '\r' | '\n')),
            "This file is not plain text"
        );
        Ok(Self {
            original,
            text: initial.clone(),
            initial,
        })
    }

    pub fn dirty(&self) -> bool {
        self.text != self.initial
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if !self.dirty() {
            return Ok(());
        }
        let metadata = fs::symlink_metadata(path)?;
        let permissions = metadata.permissions();
        ensure!(!permissions.readonly(), "This file is read-only");
        ensure!(
            self.text.len() as u64 <= EDIT_LIMIT,
            "Inline edits are limited to 96 KiB"
        );
        let folder = path.parent().context("The file has no parent folder")?;
        let mut file = tempfile::NamedTempFile::new_in(folder)?;
        if self.original.starts_with(b"\xef\xbb\xbf") {
            file.write_all(b"\xef\xbb\xbf")?;
        }
        if self.initial.contains("\r\n") && !self.initial.replace("\r\n", "").contains('\n') {
            file.write_all(
                self.text
                    .replace("\r\n", "\n")
                    .replace('\n', "\r\n")
                    .as_bytes(),
            )?;
        } else {
            file.write_all(self.text.as_bytes())?;
        }
        file.as_file().set_permissions(permissions)?;
        file.as_file().sync_all()?;
        ensure!(
            read_small(path)? == self.original,
            "This file changed on disk. Your edit is still open."
        );
        replace(path, file)?;
        Ok(())
    }
}

#[cfg(not(windows))]
pub(crate) fn replace(path: &Path, file: tempfile::NamedTempFile) -> Result<()> {
    file.persist(path)?;
    Ok(())
}

#[cfg(windows)]
pub(crate) fn replace(path: &Path, file: tempfile::NamedTempFile) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::ReplaceFileW;
    let wide = |path: &Path| {
        path.as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<u16>>()
    };
    let replacement = file.into_temp_path();
    let target = wide(path);
    let staged = wide(&replacement);
    // Retain Windows file attributes and access controls when replacing the content.
    let replaced = unsafe {
        ReplaceFileW(
            target.as_ptr(),
            staged.as_ptr(),
            std::ptr::null(),
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if replaced == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}

fn read_small(path: &Path) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)?;
    ensure!(
        metadata.is_file() && !crate::file_properties::is_reparse_point(&metadata),
        "Only regular files can be edited here"
    );
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(EDIT_LIMIT + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= EDIT_LIMIT,
        "Inline editing is limited to 96 KiB"
    );
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_preserves_bom_and_crlf_and_rejects_external_changes() -> Result<()> {
        let folder = tempfile::tempdir_in(std::env::current_dir()?)?;
        let path = folder.path().join("notes.txt");
        fs::write(&path, b"\xef\xbb\xbfone\r\n")?;
        let mut draft = Draft::load(&path)?;
        draft.text.push_str("two\n");
        draft.save(&path)?;
        assert_eq!(fs::read(&path)?, b"\xef\xbb\xbfone\r\ntwo\r\n");
        let mut draft = Draft::load(&path)?;
        draft.text.push_str("local change");
        fs::write(&path, b"outside change")?;
        assert!(draft.save(&path).is_err());
        assert_eq!(fs::read(&path)?, b"outside change");
        Ok(())
    }
}
