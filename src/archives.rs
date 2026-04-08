use crate::Result;
use crate::FsEntry;


use std::any::Any;
use std::io::Cursor;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::RwLock;




#[cfg(alltar)]
use tar::Archive as TarArchive;

#[cfg(tgz)]
use flate2::read::GzDecoder;

#[cfg(txz)]
use xz2::read::XzDecoder;

#[cfg(zip)]
use zip::ZipArchive;

#[cfg(sevenz)]
use sevenz_rust2::{ArchiveReader, Password};





pub static ARCHIVE_EXTS: &[&str] = &[ ".7z", ".zip", ".tar", ".tgz", ".txz", ".tar.gz", ".tar.xz" ];





enum AchiveType {
    Tar,
    TarGz,
    TarXz,
    Zip,
    #[cfg(sevenz)]
    SevenZ
}


impl AchiveType {

    pub fn from_path<P: AsRef<Path>>(path: P) -> Option<AchiveType> {
        let path = path.as_ref().to_str().unwrap();
        if path.ends_with(".tar") {
            return Some(AchiveType::Tar);
        }
        if path.ends_with(".tar.gz") || path.ends_with(".tgz") {
            return Some(AchiveType::TarGz);
        }
        if path.ends_with(".tar.xz") || path.ends_with(".txz") {
            return Some(AchiveType::TarXz);
        }
        if path.ends_with(".zip") {
            return Some(AchiveType::Zip);
        }
        #[cfg(sevenz)]
        if path.ends_with(".7z") {
            return Some(AchiveType::SevenZ);
        }
        None
    }
    
}






pub trait AchiveExt {
    fn is_archive(&self) -> bool;
    fn archive(&self) -> Result<Archive>;
}

impl AchiveExt for FsEntry {
    fn is_archive(&self) -> bool {
        for ext in ARCHIVE_EXTS.iter() {
            if self.path.to_str().unwrap().ends_with(ext) {
                return true;
            }
        }
        false
    }

    fn archive(&self) -> Result<Archive> {
        if !self.is_archive() {
            return Err(crate::Error::Unknown(format!("archive or feature is not active could not open: {}", self.path.display().to_string())))
        }
        Ok(Archive::new(self.path.as_path(), self.content()?))
    }
}








pub struct ArchiveEntry<'a> {
    size: u64,
    path: PathBuf,
    reader: ArchiveEntryReader<'a>,
}

impl<'a> ArchiveEntry<'a> {

    pub fn size(&self) -> u64 {
        self.size
    }

    pub fn path(&self) -> String {
        self.path.display().to_string()
    }

    pub fn metadata(&self) -> Result<std::fs::Metadata> {
        Ok(self.path.metadata()?)
    }

    pub fn content(&mut self) -> Result<Vec<u8>> {
        match &self.reader {
            #[cfg(tar)]
            ArchiveEntryReader::Tar { reader } => {
                let mut read_guard = reader.write().unwrap();
                let mut buf = Vec::new();
                read_guard.read_to_end(&mut buf)?;
                Ok(buf)
            },
            #[cfg(tgz)]
            ArchiveEntryReader::TarGz { reader } => {
                let mut read_guard = reader.write().unwrap();
                let mut buf = Vec::new();
                read_guard.read_to_end(&mut buf)?;
                Ok(buf)
            },
            #[cfg(txz)]
            ArchiveEntryReader::TarXz { reader } => {
                let mut read_guard = reader.write().unwrap();
                let mut buf = Vec::new();
                read_guard.read_to_end(&mut buf)?;
                Ok(buf)
            },
            #[cfg(zip)]
            ArchiveEntryReader::Zip { filename, reader } => {
                let mut read_guard = reader.write().unwrap();
                let mut file = read_guard.by_name(filename)?;
                let mut buf = Vec::new();
                file.read_to_end(&mut buf)?;
                Ok(buf)
            },
            #[cfg(sevenz)]
            ArchiveEntryReader::Sevenz { reader , ..} => {
                let mut read_guard = reader.write().unwrap();
                let mut buf = Vec::new();
                read_guard.read_to_end(&mut buf)?;
                Ok(buf)
            }
        }
    }

}

enum ArchiveEntryReader<'a> {
    #[cfg(tar)]
    Tar {
        reader: Arc<RwLock<tar::Entry<'a, Cursor<&'static [u8]>>>>
    },
    #[cfg(tgz)]
    TarGz {
        reader: Arc<RwLock<tar::Entry<'a, GzDecoder<Cursor<&'static [u8]>>>>>
    },
    #[cfg(txz)]
    TarXz {
        reader: Arc<RwLock<tar::Entry<'a, XzDecoder<Cursor<&'static [u8]>>>>>
    },
    #[cfg(zip)]
    Zip {
        filename: &'a str,
        reader: Arc<RwLock<&'a mut ZipArchive<std::io::Cursor<&'static [u8]>>>>
    },
    #[cfg(sevenz)]
    Sevenz {
        reader: Arc<RwLock<&'a mut dyn Read>>
    }
}








pub struct Archive {
    path: PathBuf,
    data: &'static [u8],
    kind: Option<AchiveType>,
}

impl<'a> Archive {

    pub fn new<P: AsRef<Path>>(path: P, data: &'static [u8]) -> Self {
        Self {
            data: data,
            path: path.as_ref().to_path_buf(),
            kind: AchiveType::from_path(path.as_ref()),
        }
    }

    fn open(&self) -> Box<dyn Any> {
        let cursor = Cursor::new(self.data);
        match self.kind.as_ref().unwrap() {
            #[cfg(tar)]
            AchiveType::Tar => Box::new(TarArchive::new(cursor)),
            #[cfg(tgz)]
            AchiveType::TarGz => Box::new(TarArchive::new(GzDecoder::new(cursor))),
            #[cfg(txz)]
            AchiveType::TarXz => Box::new(TarArchive::new(XzDecoder::new(cursor))),
            #[cfg(zip)]
            AchiveType::Zip => Box::new(ZipArchive::new(cursor).unwrap()),
            #[cfg(sevenz)]
            AchiveType::SevenZ => Box::new(ArchiveReader::new(cursor, Password::empty()).unwrap()),
        }
    }

    pub fn metadata(&self) -> Result<std::fs::Metadata> {
        Ok(self.path.metadata()?)
    }

    pub fn entries<F: FnMut(ArchiveEntry)>(&self, mut callback: F) -> Result<()> {
        #[cfg(tar)]
        if let Some(archive) = self.open().downcast_mut::<TarArchive<Cursor<&'static [u8]>>>() {
            for entry in archive.entries()?.flatten() {
                let path = entry.path()?.display().to_string();
                let size = entry.size();
                if !entry.path()?.is_dir() {
                    callback(self.create_archive_entry(&path, size, ArchiveEntryReader::Tar { reader: Arc::new(RwLock::new(entry)) }));
                }
            }
        }
        #[cfg(tgz)]
        if let Some(archive) = self.open().downcast_mut::<TarArchive<GzDecoder<Cursor<&'static [u8]>>>>() {
            for entry in archive.entries()?.flatten() {
                let path = entry.path()?.display().to_string();
                let size = entry.size();
                if !entry.path()?.is_dir() {
                    callback(self.create_archive_entry(&path, size, ArchiveEntryReader::TarGz { reader: Arc::new(RwLock::new(entry)) }));
                }
            }
        }
        #[cfg(txz)]
        if let Some(archive) = self.open().downcast_mut::<TarArchive<XzDecoder<Cursor<&'static [u8]>>>>() {
            for entry in archive.entries()?.flatten() {
                let path = entry.path()?.display().to_string();
                let size = entry.size();
                if !entry.path()?.is_dir() {
                    callback(self.create_archive_entry(&path, size, ArchiveEntryReader::TarXz { reader: Arc::new(RwLock::new(entry)) }));
                }
            }
        }
        #[cfg(zip)]
        if let Some(archive) = self.open().downcast_mut::<ZipArchive<Cursor<&'static [u8]>>>() {
            let len = archive.len();
            for cur in 0..len {
                let mut archive_entry = archive.clone();
                if let Some(entry) = archive_entry.by_index(cur).ok() {
                    if !entry.is_dir() {
                        let size = entry.size();
                        callback(self.create_archive_entry(entry.name(), size, ArchiveEntryReader::Zip { filename: entry.name(), reader: Arc::new(RwLock::new(archive)) }));
                    }
                }
            }
        }
        #[cfg(sevenz)]
        if let Some(archive) = self.open().downcast_mut::<ArchiveReader<Cursor<&'static [u8]>>>() {
            archive.for_each_entries(|entry, read| {
                callback(self.create_archive_entry(&entry.name, entry.size, ArchiveEntryReader::Sevenz { reader: Arc::new(RwLock::new(read)) }));
                Ok(true)
            })?
        }
        Ok(())
    }


    fn create_archive_entry(&'a self, path: &str, size: u64, reader: ArchiveEntryReader<'a>) -> ArchiveEntry<'a> {
        ArchiveEntry {
            size,
            path: path.replace("./", "").replace("\\", "/").into(),
            reader: reader
        }
    }

}