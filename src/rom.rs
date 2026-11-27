//! ROM loading and LoROM address mapping.

use crate::crc32::crc32;
use std::fs;
use std::path::Path;

const COPIER_HEADER_LEN: usize = 0x200;
const LOROM_HEADER: usize = 0x7FC0;

pub struct Rom {
    data: Vec<u8>,
    pub had_copier_header: bool,
}

pub struct Header {
    pub title: String,
    pub map_mode: u8,
    pub rom_type: u8,
    pub country: u8,
    pub version: u8,
}

#[derive(Debug, Clone, Copy)]
pub struct Vectors {
    pub native_cop: u16,
    pub native_brk: u16,
    pub native_nmi: u16,
    pub native_irq: u16,
    pub emu_cop: u16,
    pub emu_nmi: u16,
    pub emu_reset: u16,
    pub emu_irq: u16,
}

impl Rom {
    pub fn load(path: impl AsRef<Path>) -> Result<Rom, String> {
        let path = path.as_ref();
        let data = fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        Rom::from_bytes(data)
    }

    /// Accepts headered (.smc) and unheadered (.sfc) dumps.
    pub fn from_bytes(mut data: Vec<u8>) -> Result<Rom, String> {
        if let Some(kind) = archive_kind(&data) {
            return Err(format!("this file is a {kind} archive, not a ROM: extract the .sfc or .smc file from it first"));
        }
        let had_copier_header = data.len() % 0x8000 == COPIER_HEADER_LEN;
        if had_copier_header {
            data.drain(..COPIER_HEADER_LEN);
        }
        if data.len() < 0x8000 {
            return Err(format!("this file is too small to be a ROM ({} bytes): is it the whole game?", data.len()));
        }
        Ok(Rom { data, had_copier_header })
    }

    /// What to tell a person whose ROM no profile knows: what the file is, and the usual reasons for a different CRC32.
    pub fn describe_unknown(&self) -> String {
        let mut m = format!("Your file: {} bytes", self.data.len());
        if self.had_copier_header {
            m.push_str(" (a 512-byte copier header was removed before the check)");
        }
        let h = self.header();
        let region = match h.country {
            0 => "Japan",
            1 => "USA",
            2 => "Europe",
            3 => "Sweden",
            4 => "Finland",
            5 => "Denmark",
            6 => "France",
            7 => "Netherlands",
            8 => "Spain",
            9 => "Germany",
            0x0A => "Italy",
            _ => "unknown",
        };
        let title_ok = h.title.chars().filter(|c| *c != '?').count() * 2 >= h.title.chars().count() && !h.title.is_empty();
        if title_ok {
            m.push_str(&format!(", internal title \"{}\", region code {:02X} ({region}).\n", h.title, h.country));
        } else {
            m.push_str(".\n");
        }
        if self.data.len() % 0x8000 != 0 {
            m.push_str("The size is not a whole number of 32 KiB banks: the file looks cut short or altered.\n");
        }
        m.push_str("The game needs an unmodified dump of one of the versions below. Another region or revision, a translation patch or a ");
        m.push_str("ROM hack has a different CRC32 and is refused.\n");
        m
    }

    /// The image without the copier header.
    pub fn bytes(&self) -> &[u8] {
        &self.data
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn crc32(&self) -> u32 {
        crc32(&self.data)
    }

    /// File offset for a 24-bit LoROM address, or None for RAM / hardware / unmapped regions.
    pub fn lorom_offset(&self, addr: u32) -> Option<usize> {
        let bank = ((addr >> 16) & 0xFF) as usize;
        let off = (addr & 0xFFFF) as usize;
        if off < 0x8000 || bank == 0x7E || bank == 0x7F {
            return None;
        }
        let file_off = ((bank & 0x7F) << 15) | (off & 0x7FFF);
        Some(file_off % self.data.len())
    }

    pub fn read8(&self, addr: u32) -> Option<u8> {
        self.lorom_offset(addr).map(|o| self.data[o])
    }

    pub fn read16(&self, addr: u32) -> Option<u16> {
        let lo = self.read8(addr)? as u16;
        let hi = self.read8(addr.wrapping_add(1) & 0xFF_FFFF)? as u16;
        Some(lo | (hi << 8))
    }

    pub fn header(&self) -> Header {
        let h = &self.data[LOROM_HEADER..LOROM_HEADER + 0x20];
        let title = h[..21]
            .iter()
            .map(|&b| if (0x20..0x7F).contains(&b) { b as char } else { '?' })
            .collect::<String>()
            .trim_end()
            .to_string();
        Header { title, map_mode: h[0x15], rom_type: h[0x16], country: h[0x19], version: h[0x1B] }
    }

    pub fn vectors(&self) -> Vectors {
        let v = |off: usize| u16::from_le_bytes([self.data[0x7F00 + off], self.data[0x7F00 + off + 1]]);
        Vectors {
            native_cop: v(0xE4),
            native_brk: v(0xE6),
            native_nmi: v(0xEA),
            native_irq: v(0xEE),
            emu_cop: v(0xF4),
            emu_nmi: v(0xFA),
            emu_reset: v(0xFC),
            emu_irq: v(0xFE),
        }
    }
}

/// The kind of archive a file is, if its first bytes say so (people often point the launcher at the .zip they downloaded).
pub fn archive_kind(data: &[u8]) -> Option<&'static str> {
    const KINDS: [(&[u8], &str); 5] = [(&[0x50, 0x4B, 0x03, 0x04], "ZIP"), (&[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C], "7-Zip"), (b"Rar!", "RAR"), (&[0x1F, 0x8B], "gzip"), (b"BZh", "bzip2")];
    KINDS.iter().find(|(magic, _)| data.starts_with(magic)).map(|(_, k)| *k)
}

#[cfg(test)]
pub(crate) fn test_rom(code: &[(u32, &[u8])]) -> Rom {
    let mut data = vec![0u8; 0x10000];
    for &(addr, bytes) in code {
        let off = ((addr as usize >> 16) & 0x7F) << 15 | (addr as usize & 0x7FFF);
        data[off..off + bytes.len()].copy_from_slice(bytes);
    }
    Rom::from_bytes(data).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lorom_mapping() {
        let rom = Rom::from_bytes(vec![0; 0x10000]).unwrap();
        assert_eq!(rom.lorom_offset(0x00_8000), Some(0));
        assert_eq!(rom.lorom_offset(0x80_8000), Some(0));
        assert_eq!(rom.lorom_offset(0x01_8000), Some(0x8000));
        assert_eq!(rom.lorom_offset(0x00_2100), None);
        assert_eq!(rom.lorom_offset(0x7E_8000), None);
    }

    #[test]
    fn an_archive_is_not_a_rom_and_an_unknown_one_is_described() {
        let mut zip = b"PK\x03\x04".to_vec();
        zip.resize(0x9000, 0);
        let e = Rom::from_bytes(zip).err().unwrap();
        assert!(e.contains("ZIP archive") && e.contains("extract"), "{e}");
        assert!(Rom::from_bytes(b"nope".to_vec()).err().unwrap().contains("too small"));
        let d = Rom::from_bytes(vec![0; 0x8000 + 0x200]).unwrap().describe_unknown();
        assert!(d.contains("copier header was removed") && !d.contains("internal title") && !d.contains("cut short"), "{d}");
        let d = Rom::from_bytes(vec![0; 0x8005]).unwrap().describe_unknown();
        assert!(d.contains("cut short") && !d.contains("copier"), "{d}");
        let d = Rom::from_bytes(vec![0; 0x10000]).unwrap().describe_unknown();
        assert!(!d.contains("cut short") && !d.contains("copier"), "{d}");
    }

    #[test]
    fn strips_copier_header() {
        let rom = Rom::from_bytes(vec![0; 0x8000 + 0x200]).unwrap();
        assert!(rom.had_copier_header);
        assert_eq!(rom.len(), 0x8000);
    }
}
