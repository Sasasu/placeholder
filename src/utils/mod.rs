use log::*;
use std::collections::HashMap;
use std::process::Command;

const HEX_CHARS: &[u8] = b"0123456789ABCDEF";

/// convent bytes to hex string (for debug or dump)
pub fn bytes_to_hex(bytes: &[u8]) -> String {
    let mut v: Vec<u8> = Vec::with_capacity(bytes.len() * 2);
    for &bytes in bytes {
        v.push(HEX_CHARS[(bytes >> 4) as usize]);
        v.push(HEX_CHARS[(bytes & 0xf) as usize]);
    }
    unsafe { String::from_utf8_unchecked(v) }
}

pub struct Reader;

impl Reader {
    pub fn read_u4_high(bytes: &[u8]) -> u8 {
        bytes[0] >> 4
    }

    pub fn read_u4_low(bytes: &[u8]) -> u8 {
        bytes[0] & 0x0fu8
    }

    pub fn read_u8(bytes: &[u8]) -> u8 {
        bytes[0]
    }

    pub fn read_u16(bytes: &[u8]) -> u16 {
        u16::from(bytes[0]) << 8 | u16::from(bytes[1])
    }
}

pub struct Writer;

impl Writer {
    pub fn write_u8(buff: &mut [u8], value: u8) {
        buff[0] = value;
    }
}

pub fn run_command<S: ::std::hash::BuildHasher>(env: &HashMap<String, String, S>, command: &str) {
    for i in command.split('\n').filter(|x| !x.is_empty()) {
        info!("running {:?}", &i);
        let code = Command::new("sh")
            .envs(env)
            .arg("-c")
            .arg(i)
            .spawn()
            .expect("device command exec failure")
            .wait()
            .unwrap()
            .code()
            .unwrap();
        assert_eq!(code, 0);
    }
}
