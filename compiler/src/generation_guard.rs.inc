{
    let admission = (|| -> ::std::io::Result<::std::fs::File> {
        use ::std::io::Read as _;
        let executable = ::std::env::current_exe()?;
        let generation = executable.parent().ok_or_else(|| ::std::io::Error::other("missing generation directory"))?;
        if generation.file_name() != Some(::std::ffi::OsStr::new(@GENERATION@)) {
            return Err(::std::io::Error::other("temporary executable moved outside its generation"));
        }
        let app = generation.parent().and_then(::std::path::Path::parent)
            .ok_or_else(|| ::std::io::Error::other("missing application directory"))?;
        if app.file_name() != Some(::std::ffi::OsStr::new(@APPLICATION@)) {
            return Err(::std::io::Error::other("temporary executable has a different application"));
        }
        let path = app.join("run-retention").join(concat!(@GENERATION@, ".lease"));
        let metadata = ::std::fs::symlink_metadata(&path)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(::std::io::Error::other("run lease is not a plain file"));
        }
        #[cfg(windows)] {
            use ::std::os::windows::fs::MetadataExt as _;
            if metadata.file_attributes() & 0x400 != 0 {
                return Err(::std::io::Error::other("run lease is a reparse point"));
            }
        }
        let mut file = ::std::fs::OpenOptions::new().read(true).write(true).open(&path)?;
        match file.try_lock_shared() {
            Ok(()) => {},
            Err(::std::fs::TryLockError::WouldBlock) => {
                eprintln!("waiting for Nagi run lease: {}", path.display());
                file.lock_shared()?;
            },
            Err(::std::fs::TryLockError::Error(error)) => return Err(error),
        }
        let mut header = ::std::vec::Vec::new();
        (&mut file).take(4097).read_to_end(&mut header)?;
        if header != @READY@.as_bytes() {
            return Err(::std::io::Error::other("temporary run generation is retired or its lease is invalid"));
        }
        Ok(file)
    })();
    match admission {
        // Keep the native pin through TLS shutdown, panic and process::exit.
        // The OS, rather than ordinary Rust scope Drop, closes it at process exit.
        Ok(file) => ::std::mem::forget(file),
        Err(error) => {
            eprintln!("Nagi cached run startup refused: {error}");
            ::std::process::exit(1);
        },
    }
}
