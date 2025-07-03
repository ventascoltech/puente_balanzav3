use parking_lot::Mutex;
use std::sync::Arc;
use std::time::{Instant};

use crate::serial_utils::sanitize_log_data;

pub type SharedCache = Arc<Mutex<Cache>>;

pub struct Cache {
    data: Option<(Vec<u8>, Instant)>,
}

impl Cache {
    /// Crea una nueva instancia vacía
    pub fn new() -> Self {
        Self { data: None }
    }

    /// Establece nuevos datos con su timestamp
    pub fn set(&mut self, data: Vec<u8>) {
        self.data = Some((data, Instant::now()));
    }

    /// Permite acceder a los datos y su timestamp (uso interno controlado)
    pub fn get_raw(&self) -> Option<(&[u8], Instant)> {
        self.data.as_ref().map(|(d, t)| (d.as_slice(), *t))
    }

}

impl Default for Cache {
    fn default() -> Self {
        Self::new()
    }
}

