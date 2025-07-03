use serialport::{DataBits, Parity, StopBits};

use anyhow::Result;
// === src/serial_utils.rs ===
use serde::{self, Deserialize, Deserializer};

pub fn deserialize_data_bits<'de, D>(deserializer: D) -> Result<DataBits, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    match s.as_str() {
        "5" => Ok(DataBits::Five),
        "6" => Ok(DataBits::Six),
        "7" => Ok(DataBits::Seven),
        "8" => Ok(DataBits::Eight),
        _ => Err(serde::de::Error::custom("data_bits inválido")),
    }
}

pub fn deserialize_parity<'de, D>(deserializer: D) -> Result<Parity, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    match s.to_lowercase().as_str() {
        "none" => Ok(Parity::None),
        "odd" => Ok(Parity::Odd),
        "even" => Ok(Parity::Even),
        _ => Err(serde::de::Error::custom("parity inválido")),
    }
}

pub fn deserialize_stop_bits<'de, D>(deserializer: D) -> Result<StopBits, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    match s.as_str() {
        "1" => Ok(StopBits::One),
        "2" => Ok(StopBits::Two),
        _ => Err(serde::de::Error::custom("stop_bits inválido")),
    }
}


/// Verifica si los datos recibidos son relevantes o deben descartarse.
pub fn is_relevant_data(data: &[u8]) -> bool {
    const IRRELEVANT_PATTERNS: &[&[u8]] = &[
        &[0x18, 0x0D],
        &[0x02, 0x3F, 0x58, 0x0D], // ?X
        &[0x02, 0x3F, 0x50, 0x0D], // ?P
        &[0x02, 0x3F, 0x44, 0x0D], // ?D
        &[0x02, 0x3F, 0x41, 0x0D], // ?A
        b"00000",
    ];
    const ENDS_WITH_PATTERN: &[u8] = b"0.005\r";
    const CONTAINS_PATTERN: &[u8] = b"Count        Weight/kg";

    !IRRELEVANT_PATTERNS.iter().any(|pat| data == *pat)
        && !data.ends_with(ENDS_WITH_PATTERN)
        && !data.windows(CONTAINS_PATTERN.len()).any(|w| w == CONTAINS_PATTERN)
}


/// Devuelve una cadena legible seguida del contenido con caracteres escapados,
/// asegurando que el resultado sea completamente imprimible para journald.
/// 


/// Convierte datos binarios en una representación legible para logs.
pub fn sanitize_log_data(data: &[u8]) -> String {
    data.iter()
        .filter_map(|&byte| {
            // Ignorar completamente caracteres molestos como DEL
            if byte == 0x7F {
                None
            } else if byte.is_ascii_graphic() || byte == b' ' {
                Some((byte as char).to_string())
            } else if byte == b'\r' {
                Some("\\r".to_string())
            } else if byte == b'\n' {
                Some("\\n".to_string())
            } else {
                Some(format!("\\x{:02X}", byte))
            }
        })
        .collect()
}




/*
pub fn sanitize_log_data(data: &[u8]) -> String {
    let readable: String = data
        .iter()
        .filter_map(|&byte| {
            if byte == 0x7F {
                None
            } else if byte.is_ascii_graphic() || byte == b' ' {
                Some(byte as char)
            } else if byte == b'\r' {
                Some('\\');
                Some('r')
            } else if byte == b'\n' {
                Some('\\');
                Some('n')
            } else {
                None
            }
        })
        .collect();

    let escaped: String = data
        .iter()
        .map(|&byte| match byte {
            b if b.is_ascii_graphic() || b == b' ' => (b as char).to_string(),
            b'\r' => "\\r".to_string(),
            b'\n' => "\\n".to_string(),
            _ => format!("\\x{:02X}", byte),
        })
        .collect();

    format!("[{}] bytes: {}", readable, escaped)
}

*/
