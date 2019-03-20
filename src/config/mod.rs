use crate::interface::Type;
use clap::{App, Arg, ArgMatches};
use log::*;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use serde::{self, Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::net::Ipv4Addr;
use std::path::Path;

pub mod global;

pub use self::global::{ARG, CONFIG};

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    /// device name, will be used in `$INTERFACE`
    /// like "ph0"
    pub device_name: String,

    /// device type, only `tun` or `tap`
    /// but tap is not implemented
    pub device_type: Type,

    /// listen port, both TCP and UDP will be listen
    pub port: u16,

    /// self subnet like "127.0.0.0/24"
    pub subnet: String,

    /// can use `$INTERFACE` `$IP_ADDR_MASK` as device name and ip address
    /// see `config.example.yaml`
    pub ifup: String,

    /// same as `ifup`
    pub ifdown: String,

    /// the server to connect when setup
    pub servers: Vec<Server>,

    /// myself name
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Server {
    /// server public address
    pub address: String,
    pub port: u16,
    pub name: String,
}

impl Config {
    pub fn get() -> &'static Self {
        &CONFIG
    }
}

impl Config {
    pub fn from_path<P>(path: P) -> Self
    where
        P: AsRef<Path> + Send + 'static,
    {
        let data = fs::read(path);
        let config = serde_yaml::from_slice(&data.unwrap()).unwrap_or_else(|e| {
            log::warn!("decode error, {}, fall back to default config", e);
            Self::default()
        });
        info!("loaded config: {:?}", config);
        config
    }

    pub fn get_env(&self) -> HashMap<String, String> {
        let mut h = HashMap::with_capacity(2);
        h.insert("INTERFACE".to_string(), self.device_name.clone());
        h.insert("IP_ADDR_MASK".to_string(), self.subnet.clone());
        h
    }
}

impl Config {
    pub fn get_v4(&self) -> Ipv4Addr {
        let t: Vec<&str> = self.subnet.split('/').collect();
        let t = t[0];
        t.parse().unwrap()
    }

    pub fn get_v4_mask(&self) -> u32 {
        let t: Vec<&str> = self.subnet.split('/').collect();
        let t = t[1];
        t.parse().unwrap()
    }
}

impl Default for Config {
    fn default() -> Self {
        let c = Config {
            device_name: "ph0".to_string(),
            device_type: Type::Tun,
            port: 7654,
            subnet: "128.66.1.0/32".to_string(),
            ifup: "/bin/sh -c 'exit 1'".to_string(),
            ifdown: "/bin/sh -c 'exit 1'".to_string(),
            servers: vec![],
            name: "ph-".to_string()
                + thread_rng()
                    .sample_iter(&Alphanumeric)
                    .take(5)
                    .collect::<String>()
                    .as_str(),
        };
        info!("loaded default config: {:?}", c);
        c
    }
}
