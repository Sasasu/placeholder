use crate::utils;
use log::*;
use rand::AsByteSliceMut;
use std::collections::linked_list::LinkedList;
use std::convert::Into;
use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::RwLock;

pub struct Buffer;

#[derive(Debug)]
pub enum Version {
    V4,
    V6,
}

pub struct Package {
    // IpVersion,
    // SourceAddress,
    // DestinationAddress,
    // ...
    // RawData,
    pub raw_package: Vec<u8>,
}

impl Package {
    #[inline]
    pub fn as_slice(&mut self) -> &mut [u8] {
        assert_eq!(self.raw_package.capacity() >= 1500, true);
        unsafe {
            self.raw_package.set_len(1500);
        }
        self.raw_package.as_byte_slice_mut()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.raw_package.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.raw_package.is_empty()
    }

    #[inline]
    pub fn version(&self) -> Version {
        match utils::Reader::read_u4_high(self.raw_package.as_ref()) {
            4 => Version::V4,
            6 => Version::V6,
            _ => unreachable!("bad ip package: {:?}", self.raw_package),
        }
    }

    #[inline]
    pub fn source_address(&self) -> IpAddr {
        let r = &self.raw_package;
        match self.version() {
            Version::V4 => {
                assert!(r.len() >= 15);
                IpAddr::V4(Ipv4Addr::from([r[12], r[13], r[14], r[15]]))
            }
            Version::V6 => {
                assert!(r.len() >= 23);
                IpAddr::V6(Ipv6Addr::from([
                    r[8], r[9], r[10], r[11], r[12], r[13], r[14], r[15], r[16], r[17], r[18],
                    r[19], r[20], r[21], r[22], r[23],
                ]))
            }
        }
    }

    #[inline]
    pub fn destination_address(&self) -> IpAddr {
        let r = &self.raw_package;
        match self.version() {
            Version::V4 => {
                assert!(r.len() >= 20);
                IpAddr::V4(Ipv4Addr::from([r[16], r[17], r[18], r[19]]))
            }
            Version::V6 => {
                assert!(r.len() >= 40);
                IpAddr::V6(Ipv6Addr::from([
                    r[24], r[25], r[26], r[27], r[28], r[29], r[30], r[31], r[32], r[33], r[34],
                    r[35], r[36], r[37], r[38], r[39],
                ]))
            }
        }
    }
    #[inline]
    pub fn from_buffer(buffer: Vec<u8>) -> Self {
        Package {
            raw_package: buffer,
        }
    }
}

impl Buffer {
    pub fn get() -> Vec<u8> {
        let mut b = BUFFER.write().unwrap();
        if b.is_empty() {
            trace!("buffer empty, push new buffer");
            b.push_back(vec![0; 1500]);
            b.push_back(vec![0; 1500]);
            b.push_back(vec![0; 1500]);
            b.push_back(vec![0; 1500]);
            b.push_back(vec![0; 1500]);
            return vec![0; 1500];
        }

        b.pop_front().unwrap()
    }

    #[inline]
    pub fn set_len(buffer: &mut Vec<u8>, len: usize) {
        unsafe { buffer.set_len(len) }
    }

    #[inline]
    pub fn put_back(buffer: Vec<u8>) {
        let mut b = BUFFER.write().unwrap();
        b.push_back(buffer)
    }
}

impl fmt::Debug for Package {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Package {{ Version: {:?} source: {:?} destination: {:?} raw_package: {{ ...{} bytes data... }} }}",
            self.version(),
            self.source_address(),
            self.destination_address(),
            self.raw_package.len(),
        )
    }
}

impl fmt::Display for Package {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Package {} source={} destination={} data={{ ...{} bytes data... }}",
            self.version(),
            self.source_address(),
            self.destination_address(),
            self.raw_package.len(),
        )
    }
}

impl Into<Vec<u8>> for Package {
    fn into(self) -> Vec<u8> {
        self.raw_package
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Version::V4 => write!(f, "v4"),
            Version::V6 => write!(f, "v6"),
        }
    }
}

lazy_static! {
    static ref BUFFER: RwLock<LinkedList<Vec<u8>>> = LinkedList::new().into();
}
