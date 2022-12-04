use std::{error::Error, io::BufWriter, path::Path};

use serde::{Deserialize, Serialize};
use std::{fs::File, io::BufReader};

pub struct JsonStore<P> {
    path: P,
}

impl<P: AsRef<Path>> JsonStore<P> {
    pub fn new(path: P) -> Self {
        Self { path }
    }

    pub fn read<T: for<'a> Deserialize<'a>>(&self) -> Result<T, Box<dyn Error + Sync + Send>> {
        if !self.path.as_ref().is_file() {
            File::create(&self.path)?;
        }
        let file = File::open(&self.path)?;
        let reader = BufReader::new(file);
        let v = serde_json::from_reader(reader)?;
        Ok(v)
    }

    pub fn write<T: Serialize>(&self, value: &T) -> Result<(), Box<dyn Error + Sync + Send>> {
        let file = File::create(&self.path)?;
        let writer = BufWriter::new(file);
        serde_json::to_writer_pretty(writer, value)?;
        Ok(())
    }
}
