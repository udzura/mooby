extern crate simple_endian;

use core::ascii;
use core::ffi::CStr;
use core::mem;

use crate::error::Error;

use plain::Plain;
use simple_endian::{u16be, u32be};

#[repr(C)]
#[derive(Debug)]
pub struct RiteBinaryHeader {
    pub ident: [u8; 4],
    pub major_version: [u8; 2],
    pub minor_version: [u8; 2],
    pub size: [u8; 4],
    pub compiler_name: [u8; 4],
    pub compiler_version: [u8; 4],
}
unsafe impl Plain for RiteBinaryHeader {}

impl RiteBinaryHeader {
    fn from_bytes(buf: &[u8]) -> Result<&Self, Error> {
        plain::from_bytes(buf).map_err(|_| Error::General)
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct SectionMiscHeader {
    pub ident: [u8; 4],
    pub size: [u8; 4],
}
unsafe impl Plain for SectionMiscHeader {}

impl SectionMiscHeader {
    fn from_bytes(buf: &[u8]) -> Result<&Self, Error> {
        plain::from_bytes(buf).map_err(|_| Error::General)
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct SectionIrepHeader {
    pub ident: [u8; 4],
    pub size: [u8; 4],

    pub rite_version: [u8; 4],
}
unsafe impl Plain for SectionIrepHeader {}

impl SectionIrepHeader {
    fn from_bytes(buf: &[u8]) -> Result<&Self, Error> {
        plain::from_bytes(buf).map_err(|_| Error::General)
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct IrepRecord {
    pub size: [u8; 4],
    pub nlocals: [u8; 2],
    pub nregs: [u8; 2],
    pub rlen: [u8; 2],
    pub clen: [u8; 2],
    pub ilen: [u8; 4],
}

unsafe impl Plain for IrepRecord {}

impl IrepRecord {
    fn from_bytes(buf: &[u8]) -> Result<&Self, Error> {
        plain::from_bytes(buf).map_err(|_| Error::General)
    }
}

pub fn load(src: &[u8]) -> Result<(), Error> {
    let mut size = src.len();
    let mut head = src;
    let binheader_size = mem::size_of::<RiteBinaryHeader>();
    if size < binheader_size {
        return Err(Error::TooShort);
    }
    let bin_header = RiteBinaryHeader::from_bytes(&head[0..binheader_size])?;
    size -= binheader_size;
    head = &head[binheader_size..];

    dbg!(bin_header);
    let binsize: u32 = be32_to_u32(bin_header.size);
    eprintln!("size {}", binsize);

    let irep_header_size = mem::size_of::<SectionIrepHeader>();
    if size < irep_header_size {
        return Err(Error::TooShort);
    }

    let mut irep_size: usize = 0;
    use ascii::Char::*;
    loop {
        match peek4(head) {
            Some(chrs) => match chrs {
                [CapitalI, CapitalR, CapitalE, CapitalP] => {
                    let cur = section_irep_1(head)?;
                    head = &head[cur..];
                }
                [CapitalE, CapitalN, CapitalD, Null] => {
                    let cur = section_end(head)?;
                    head = &head[cur..];
                }
                _ => {
                    dbg!(chrs);
                    dbg!(head);
                    return Err(Error::InvalidFormat);
                }
            },
            None => {
                break;
            }
        }
    }

    Ok(())
}

pub fn section_irep_1(head: &[u8]) -> Result<usize, Error> {
    let mut cur = 0;

    let irep_header_size = mem::size_of::<SectionIrepHeader>();
    let irep_header = SectionIrepHeader::from_bytes(&head[cur..irep_header_size])?;
    let irep_size = be32_to_u32(irep_header.size) as usize;
    if head.len() < irep_size {
        return Err(Error::TooShort);
    }
    cur += irep_header_size;

    while cur < irep_size {
        let start_cur = cur;
        // insn
        let record_size = mem::size_of::<IrepRecord>();
        let irep_record = IrepRecord::from_bytes(&head[cur..cur + record_size])?;
        let irep_rec_size = be32_to_u32(irep_record.size) as usize;
        let ilen = be32_to_u32(irep_record.ilen) as usize;
        dbg!(ilen);
        cur += record_size;

        let insns = &head[cur..cur + ilen];
        // dbg!(insns);

        cur += ilen;

        // pool
        let data = &head[cur..cur + 2];
        let plen = be16_to_u16([data[0], data[1]]) as usize;
        cur += 2;
        dbg!(plen);
        for _ in 0..plen {
            let typ = head[cur];
            match typ {
                0 => {
                    cur += 1;
                    let data = &head[cur..cur + 2];
                    let strlen = be16_to_u16([data[0], data[1]]) as usize + 1;
                    cur += 2;
                    let strval = CStr::from_bytes_with_nul(&head[cur..cur + strlen])
                        .or(Err(Error::InvalidFormat))?;
                    dbg!(strval);
                    cur += strlen;
                }
                _ => {
                    unimplemented!("more support pool type");
                }
            }
        }

        // syms
        let data = &head[cur..cur + 2];
        let slen = be16_to_u16([data[0], data[1]]) as usize;
        cur += 2;
        dbg!(slen);
        for _ in 0..slen {
            let data = &head[cur..cur + 2];
            let symlen = be16_to_u16([data[0], data[1]]) as usize + 1;
            cur += 2;
            let symval = CStr::from_bytes_with_nul(&head[cur..cur + symlen])
                .or(Err(Error::InvalidFormat))?;
            dbg!(symval);
            cur += symlen;
        }

        cur = start_cur + irep_rec_size;
        dbg!(cur);
        dbg!(irep_size);
    }

    Ok(irep_size)
}

pub fn section_end(head: &[u8]) -> Result<usize, Error> {
    let header = SectionMiscHeader::from_bytes(head)?;
    dbg!(header.ident.as_ascii());
    Ok(mem::size_of::<SectionMiscHeader>())
}

pub fn peek4<'a>(src: &'a [u8]) -> Option<&'a [ascii::Char]> {
    if src.len() < 4 {
        // EoD
        return None;
    }
    src[0..4].as_ascii()
}

pub fn be32_to_u32(be32: [u8; 4]) -> u32 {
    let binsize_be = unsafe { mem::transmute::<[u8; 4], u32be>(be32) };
    let binsize: u32 = binsize_be.into();
    binsize
}

pub fn be16_to_u16(be16: [u8; 2]) -> u16 {
    let binsize_be = unsafe { mem::transmute::<[u8; 2], u16be>(be16) };
    let binsize: u16 = binsize_be.into();
    binsize
}
