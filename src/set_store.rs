use std::collections::HashSet;
use std::hash::Hash;
use std::path::PathBuf;
use std::str::FromStr;

pub struct SetStore<T: ToString + FromStr + Eq + Hash + Clone> {
    set: HashSet<T>,
    path: PathBuf,
}

impl<T: ToString + FromStr + Eq + Hash + Clone> SetStore<T> {
    pub fn new(path: PathBuf) -> Result<SetStore<T>, std::io::Error> {
        let mut set = HashSet::new();
        if !path.is_file() {
            std::fs::write(&path, "")?;
        }
        let contents = std::fs::read_to_string(&path)?;
        for line in contents.lines() {
            match line.parse::<T>() {
                Ok(value) => 
                set.insert(value.to_owned()),
                Err(_) => continue,
            };
        }
        Ok(SetStore { set, path })
    }
    pub fn containts(&self, value: T) -> bool {
        self.set.contains(&value)
    }
    pub fn insert(&mut self, value: T) -> std::io::Result<()> {
        self.set.insert(value);
        self.save()
    }
    pub fn remove(&mut self, value: T) -> std::io::Result<()> {
        self.set.remove(&value);
        self.save()
    }
    pub fn save(&self) -> std::io::Result<()> {
        let mut s = String::new();
        for v in &self.set {
            s.push_str(&v.to_string());
            s.push('\n');
        }
        std::fs::write(&self.path, s)
    }
}
