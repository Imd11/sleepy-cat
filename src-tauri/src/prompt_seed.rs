use std::{
    fs,
    fs::OpenOptions,
    io::{self, Write},
    path::Path,
};

const BUNDLED_PROMPT_LIBRARY: &str = include_str!("../../examples/prompts/prompts-zh.json");

pub(crate) fn initialize_prompt_library(
    prompt_path: &Path,
    settings_path: &Path,
) -> io::Result<bool> {
    // Settings also identify existing users whose library may be linked elsewhere.
    if settings_path.try_exists()? {
        return Ok(false);
    }
    if let Some(parent) = prompt_path.parent() {
        fs::create_dir_all(parent)?;
    }
    // Never truncate an existing library, including an intentionally empty one.
    let mut file = match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(prompt_path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => return Ok(false),
        Err(error) => return Err(error),
    };
    if let Err(error) = file
        .write_all(BUNDLED_PROMPT_LIBRARY.as_bytes())
        .and_then(|_| file.sync_all())
    {
        drop(file);
        // Only remove the file this initialization created, allowing a later retry.
        let _ = fs::remove_file(prompt_path);
        return Err(error);
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT_ID: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "sleepy-cat-prompt-seed-{}-{}",
                std::process::id(),
                NEXT_ID.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn prompt_path(&self) -> PathBuf {
            self.0.join("app-data/prompts.json")
        }
        fn settings_path(&self) -> PathBuf {
            self.0.join("app-data/settings.json")
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn first_launch_creates_the_complete_bundled_library_and_parent_directory() {
        let dir = TestDirectory::new();
        assert!(initialize_prompt_library(&dir.prompt_path(), &dir.settings_path()).unwrap());
        assert_eq!(
            fs::read_to_string(dir.prompt_path()).unwrap(),
            BUNDLED_PROMPT_LIBRARY
        );
    }

    #[test]
    fn existing_personal_library_is_preserved_byte_for_byte() {
        let dir = TestDirectory::new();
        fs::create_dir_all(dir.prompt_path().parent().unwrap()).unwrap();
        let personal = "{\"version\":4,\"containers\":[{\"title\":\"My prompt\"}]}\n";
        fs::write(dir.prompt_path(), personal).unwrap();
        assert!(!initialize_prompt_library(&dir.prompt_path(), &dir.settings_path()).unwrap());
        assert_eq!(fs::read_to_string(dir.prompt_path()).unwrap(), personal);
    }

    #[test]
    fn an_existing_zero_byte_file_is_not_replaced() {
        let dir = TestDirectory::new();
        fs::create_dir_all(dir.prompt_path().parent().unwrap()).unwrap();
        fs::write(dir.prompt_path(), "").unwrap();
        assert!(!initialize_prompt_library(&dir.prompt_path(), &dir.settings_path()).unwrap());
        assert_eq!(fs::read(dir.prompt_path()).unwrap(), b"");
    }

    #[test]
    fn an_existing_invalid_library_is_not_replaced() {
        let dir = TestDirectory::new();
        fs::create_dir_all(dir.prompt_path().parent().unwrap()).unwrap();
        fs::write(dir.prompt_path(), "keep this recovery data").unwrap();
        assert!(!initialize_prompt_library(&dir.prompt_path(), &dir.settings_path()).unwrap());
        assert_eq!(
            fs::read_to_string(dir.prompt_path()).unwrap(),
            "keep this recovery data"
        );
    }

    #[test]
    fn existing_settings_without_a_library_are_treated_as_an_existing_user() {
        let dir = TestDirectory::new();
        fs::create_dir_all(dir.settings_path().parent().unwrap()).unwrap();
        fs::write(
            dir.settings_path(),
            "{\"promptLibraryLink\":{\"mode\":\"linked\"}}",
        )
        .unwrap();
        assert!(!initialize_prompt_library(&dir.prompt_path(), &dir.settings_path()).unwrap());
        assert!(!dir.prompt_path().exists());
    }

    #[test]
    fn a_user_cleared_library_remains_empty_on_the_next_launch() {
        let dir = TestDirectory::new();
        assert!(initialize_prompt_library(&dir.prompt_path(), &dir.settings_path()).unwrap());
        let empty = "{\"version\":4,\"categories\":[],\"containers\":[],\"dividers\":[]}";
        fs::write(dir.prompt_path(), empty).unwrap();
        assert!(!initialize_prompt_library(&dir.prompt_path(), &dir.settings_path()).unwrap());
        assert_eq!(fs::read_to_string(dir.prompt_path()).unwrap(), empty);
    }

    #[test]
    fn repeated_initialization_does_not_restore_user_edits() {
        let dir = TestDirectory::new();
        assert!(initialize_prompt_library(&dir.prompt_path(), &dir.settings_path()).unwrap());
        fs::write(dir.prompt_path(), "my edited library").unwrap();
        assert!(!initialize_prompt_library(&dir.prompt_path(), &dir.settings_path()).unwrap());
        assert_eq!(
            fs::read_to_string(dir.prompt_path()).unwrap(),
            "my edited library"
        );
    }

    #[test]
    fn filesystem_errors_are_reported_without_replacing_the_blocking_file() {
        let dir = TestDirectory::new();
        fs::write(dir.0.join("app-data"), "not a directory").unwrap();
        assert!(initialize_prompt_library(&dir.prompt_path(), &dir.settings_path()).is_err());
        assert_eq!(
            fs::read_to_string(dir.0.join("app-data")).unwrap(),
            "not a directory"
        );
    }
}
