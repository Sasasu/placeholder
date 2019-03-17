use super::*;

lazy_static! {
    pub static ref ARG: ArgMatches<'static> = {
        App::new("placeholder a VPN for cloud")
            .version(clap::crate_version!())
            .author(clap::crate_authors!())
            .arg(
                Arg::with_name("verbosity")
                    .short("v")
                    .multiple(true)
                    .help("Increase message verbosity"),
            )
            .arg(
                Arg::with_name("quiet")
                    .long("quiet")
                    .short("q")
                    .help("Silence all output"),
            )
            .arg(
                Arg::with_name("file")
                    .short("f")
                    .long("file")
                    .default_value("./config.yaml")
                    .help("config file location"),
            )
            .get_matches()
    };
    pub static ref CONFIG: Config = crate::config::Config::from_path(ARG.value_of("file").unwrap());
}
